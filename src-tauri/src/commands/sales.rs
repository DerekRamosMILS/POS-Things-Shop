use std::collections::HashMap;

use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::sale::{CreateSaleDto, PaymentSplitDto, Sale, SaleFilters, SaleItem};
use crate::money::Cents;

/// Copia de los datos fiscales del cliente al momento de vender:
/// (rfc, razón social, régimen, código postal, uso de CFDI, nombre).
type DatosFiscalesCliente = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);
use crate::session::{require_admin, require_auth, SessionState};

/// Read a numeric system_config value, falling back to `default` when missing/invalid.
fn config_number(db: &rusqlite::Connection, key: &str, default: f64) -> f64 {
    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.trim().parse::<f64>().ok())
    .unwrap_or(default)
}

/// Cash register column that accumulates a given payment method.
fn register_field(method: &str) -> &'static str {
    match method {
        "card" => "total_card_sales",
        "transfer" => "total_transfer_sales",
        _ => "total_cash_sales",
    }
}

/// Turn the tender into the amount actually applied to the sale per method.
///
/// Only cash can be over-tendered (the surplus becomes change), so the non-cash
/// legs must not exceed the total. Returns the applied splits plus the change.
fn split_tender(
    payments: &[PaymentSplitDto],
    total: Cents,
) -> Result<(Vec<(String, Cents)>, Cents), String> {
    let mut non_cash = Cents::ZERO;
    let mut cash = Cents::ZERO;
    for p in payments {
        let amount = Cents::from_pesos(p.amount);
        // Un importe negativo es un error; uno en cero simplemente no aporta, y
        // rechazarlo impediría cobrar una venta que suma cero (un obsequio o una
        // línea totalmente descontada).
        if amount < Cents::ZERO {
            return Err("Una forma de pago no puede ser negativa".to_string());
        }
        if p.method == "cash" {
            cash = cash + amount;
        } else {
            non_cash = non_cash + amount;
        }
    }

    if non_cash > total {
        return Err("Los pagos que no son en efectivo superan el total de la venta".to_string());
    }
    if cash + non_cash < total {
        return Err("El monto recibido es insuficiente".to_string());
    }

    let cash_applied = (total - non_cash).clamp_non_negative();
    let change = (cash - cash_applied).clamp_non_negative();

    let mut applied: Vec<(String, Cents)> = payments
        .iter()
        .filter(|p| p.method != "cash")
        .map(|p| (p.method.clone(), Cents::from_pesos(p.amount)))
        .collect();
    if cash_applied.is_positive() {
        applied.push(("cash".to_string(), cash_applied));
    }

    // Una venta que suma cero —todo descontado, o un obsequio— no tiene ninguna
    // pierna con importe. Aun así hay que registrar con qué se "pagó", o el
    // cobro se queda sin forma de pago.
    if applied.is_empty() {
        let metodo = payments
            .first()
            .map(|p| p.method.clone())
            .unwrap_or_else(|| "cash".to_string());
        applied.push((metodo, Cents::ZERO));
    }

    Ok((applied, change))
}

/// Descuento que una promoción concede sobre las líneas a las que aplica.
///
/// Se evalúa en el servidor: el punto de venta dice *qué* promoción se usó, no
/// *cuánto* descuenta. Si la promoción no existe, está inactiva o quedó fuera de
/// vigencia, el descuento es cero y la venta se cobra completa.
fn promotion_discount(
    db: &rusqlite::Connection,
    promotion_id: i64,
    lines: &[(i64, Option<i64>, Cents)],
) -> Result<Cents, String> {
    let promo: Option<(String, f64, String, Option<i64>)> = db
        .query_row(
            "SELECT discount_type, discount_value, applies_to, target_id
             FROM promotions
             WHERE id = ?1
               AND is_active = 1
               AND date('now','localtime') BETWEEN date(start_date) AND date(end_date)",
            params![promotion_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();

    let Some((discount_type, value, applies_to, target_id)) = promo else {
        log::warn!("Promoción {} ignorada: no existe, está inactiva o venció", promotion_id);
        return Ok(Cents::ZERO);
    };

    if value <= 0.0 {
        return Ok(Cents::ZERO);
    }

    // Base: solo lo que la promoción alcanza, ya neto de descuentos de línea.
    let base: Cents = lines
        .iter()
        .filter(|(product_id, category_id, _)| match applies_to.as_str() {
            "category" => *category_id == target_id,
            "product" => Some(*product_id) == target_id,
            _ => true,
        })
        .map(|(_, _, net)| *net)
        .sum();

    if !base.is_positive() {
        return Ok(Cents::ZERO);
    }

    Ok(match discount_type.as_str() {
        "percentage" => base.percent(value.min(100.0)),
        _ => Cents::from_pesos(value).min(base),
    })
}

/// Human-readable label for a variant, e.g. "M / Negro".
fn variant_label(size: &Option<String>, color: &Option<String>) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(s) = size.as_deref() { if !s.trim().is_empty() { parts.push(s); } }
    if let Some(c) = color.as_deref() { if !c.trim().is_empty() { parts.push(c); } }
    if parts.is_empty() { "Único".to_string() } else { parts.join(" / ") }
}

#[tauri::command]
pub fn create_sale(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    cash_register_id: Option<i64>,
    data: CreateSaleDto,
) -> Result<Sale, String> {
    // The seller on record is the authenticated user, never a client-sent id.
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();
    registrar_venta(&db, user_id, cash_register_id, data)
}

/// Núcleo del cobro, con la conexión explícita.
///
/// El comando solo resuelve la sesión y toma el candado; toda la lógica de
/// dinero vive aquí para poder ejercitarla contra una base real en las pruebas,
/// que es donde importa que no haya sorpresas.
pub fn registrar_venta(
    db: &rusqlite::Connection,
    user_id: i64,
    cash_register_id: Option<i64>,
    data: CreateSaleDto,
) -> Result<Sale, String> {
    if data.items.is_empty() {
        return Err("La venta no tiene productos".to_string());
    }

    // Idempotency: a retried or double-fired charge must not become two sales.
    if let Some(ref rid) = data.client_request_id {
        if let Ok(existing_id) = db.query_row(
            "SELECT id FROM sales WHERE client_request_id = ?1",
            params![rid],
            |row| row.get::<_, i64>(0),
        ) {
            log::warn!("Cobro repetido ignorado (request {}), se devuelve la venta {}", rid, existing_id);
            return get_sale_by_id(db, existing_id);
        }
    }

    // Begin atomic transaction
    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<Sale, String> {
        // A sale requires an OPEN cash register. Derive it from the DB instead of
        // trusting the (possibly stale) id persisted in the client.
        let register_id: i64 = match crate::commands::cash_register::open_register_id(db) {
            Some(id) => id,
            None => return Err("La caja no está abierta. Ábrela antes de cobrar.".to_string()),
        };
        if let Some(cr) = cash_register_id {
            if cr != register_id {
                return Err("La caja indicada no coincide con la caja abierta.".to_string());
            }
        }

        let folio = crate::folios::siguiente(db, crate::folios::Serie::Ventas)?;

        // Resolve every line from the DB (authoritative price + cost snapshot).
        struct Line {
            product_id: i64,
            category_id: Option<i64>,
            name: String,
            sku: String,
            quantity: i32,
            unit_price: Cents,
            unit_cost: Cents,
            discount: Cents,
            line_subtotal: Cents,
            variant_id: Option<i64>,
            variant_label: Option<String>,
        }
        let mut lines: Vec<Line> = Vec::with_capacity(data.items.len());
        let mut subtotal = Cents::ZERO; // bruto, antes de cualquier descuento

        // Lo que los renglones anteriores de esta misma venta ya apartaron. Sin
        // esto, dos renglones del mismo producto se miden los dos contra la
        // existencia completa y entre ambos venden más de lo que hay.
        let mut apartado_producto: HashMap<i64, i32> = HashMap::new();
        let mut apartado_variante: HashMap<i64, i32> = HashMap::new();

        for item in &data.items {
            if item.quantity <= 0 {
                return Err("La cantidad de un producto es inválida".to_string());
            }
            let (name, sku, product_stock, price, cost, category_id): (String, String, i32, f64, f64, Option<i64>) = db
                .query_row(
                    "SELECT name, sku, stock, sale_price, purchase_price, category_id FROM products WHERE id = ?1",
                    params![item.product_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .map_err(|_| "Un producto de la venta ya no existe".to_string())?;

            // If a variant is specified, availability is checked against the variant.
            let (variant_id, variant_label, available) = if let Some(vid) = item.variant_id {
                let (size, color, vstock): (Option<String>, Option<String>, i32) = db
                    .query_row(
                        "SELECT size, color, stock FROM product_variants WHERE id = ?1 AND product_id = ?2 AND is_active = 1",
                        params![vid, item.product_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(|_| "La variante seleccionada no existe".to_string())?;
                let ya = apartado_variante.get(&vid).copied().unwrap_or(0);
                (Some(vid), Some(variant_label(&size, &color)), vstock - ya)
            } else {
                let ya = apartado_producto.get(&item.product_id).copied().unwrap_or(0);
                (None, None, product_stock - ya)
            };

            if available < item.quantity {
                let label = variant_label.clone().map(|l| format!(" ({})", l)).unwrap_or_default();
                return Err(format!(
                    "Stock insuficiente para '{}{}'. Disponible: {}, Solicitado: {}",
                    name, label, available, item.quantity
                ));
            }

            *apartado_producto.entry(item.product_id).or_insert(0) += item.quantity;
            if let Some(vid) = variant_id {
                *apartado_variante.entry(vid).or_insert(0) += item.quantity;
            }

            // Un descuento de línea nunca puede superar lo que vale la línea.
            let gross = Cents::from_pesos(price).times(item.quantity as i64);
            let discount = Cents::from_pesos(item.discount)
                .clamp_non_negative()
                .min(gross);
            let line_subtotal = gross - discount;
            subtotal = subtotal + gross;

            lines.push(Line {
                product_id: item.product_id,
                category_id,
                name,
                sku,
                quantity: item.quantity,
                unit_price: Cents::from_pesos(price),
                unit_cost: Cents::from_pesos(cost),
                discount,
                line_subtotal,
                variant_id,
                variant_label,
            });
        }

        // El importe del descuento se calcula aquí, no se acepta del cliente.
        let line_discount_total: Cents = lines.iter().map(|l| l.discount).sum();
        let promo_base: Vec<(i64, Option<i64>, Cents)> = lines
            .iter()
            .map(|l| (l.product_id, l.category_id, l.line_subtotal))
            .collect();
        let promo_discount = match data.promotion_id {
            Some(id) => promotion_discount(db, id, &promo_base)?,
            None => Cents::ZERO,
        };

        let discount_total = (line_discount_total + promo_discount).min(subtotal);
        let taxable = (subtotal - discount_total).clamp_non_negative();
        let tax_rate = config_number(db, "tax_rate", 0.0);
        let tax = taxable.percent(tax_rate);
        let total = taxable + tax;

        // Normalise the tender. A single-method sale is just a one-leg split.
        let tender: Vec<PaymentSplitDto> = if data.payments.is_empty() {
            vec![PaymentSplitDto {
                method: data.payment_method.clone(),
                amount: if data.payment_method == "cash" {
                    data.amount_paid
                } else {
                    total.to_pesos()
                },
            }]
        } else {
            data.payments.clone()
        };
        let (applied, change_amount) = split_tender(&tender, total)?;

        // "mixed" keeps reports honest when more than one method was used.
        let payment_method = if applied.len() > 1 {
            "mixed".to_string()
        } else {
            applied[0].0.clone()
        };
        let amount_paid = applied.iter().map(|(_, a)| *a).sum::<Cents>() + change_amount;

        // Insert sale
        db.execute(
            "INSERT INTO sales (folio, user_id, cash_register_id, subtotal, discount_total, tax, total, payment_method, amount_paid, change_amount, notes, customer_id, client_request_id, promotion_id, terminal_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                folio, user_id, register_id, subtotal.to_pesos(),
                discount_total.to_pesos(), tax.to_pesos(), total.to_pesos(), payment_method,
                amount_paid.to_pesos(), change_amount.to_pesos(), data.notes, data.customer_id,
                data.client_request_id, data.promotion_id, crate::folios::terminal_id(db)
            ],
        ).map_err(|e| e.to_string())?;

        let sale_id = db.last_insert_rowid();

        // Copia de los datos fiscales vigentes al momento de vender.
        if data.requiere_factura {
            let Some(customer_id) = data.customer_id else {
                return Err("Para facturar hay que elegir un cliente con datos fiscales".to_string());
            };
            let fiscal: DatosFiscalesCliente = db
                .query_row(
                    "SELECT rfc, razon_social, regimen_fiscal, cp_fiscal, uso_cfdi, name
                     FROM customers WHERE id = ?1",
                    params![customer_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .map_err(|_| "El cliente de la factura no existe".to_string())?;

            if fiscal.0.as_deref().unwrap_or("").trim().is_empty() {
                return Err(format!(
                    "'{}' no tiene RFC capturado. Complétalo en Clientes antes de facturar.",
                    fiscal.5
                ));
            }

            db.execute(
                "UPDATE sales SET requiere_factura = 1, fiscal_rfc = ?1, fiscal_razon_social = ?2,
                        fiscal_regimen = ?3, fiscal_cp = ?4, fiscal_uso_cfdi = ?5
                 WHERE id = ?6",
                params![
                    fiscal.0,
                    fiscal.1.filter(|v| !v.trim().is_empty()).unwrap_or(fiscal.5),
                    fiscal.2, fiscal.3, fiscal.4, sale_id
                ],
            ).map_err(|e| e.to_string())?;
        }

        // Insert sale items, decrease stock, record movements
        for line in &lines {
            db.execute(
                "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku, quantity, unit_price, discount, subtotal, unit_cost, variant_id, variant_label)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    sale_id, line.product_id, line.name, line.sku,
                    line.quantity, line.unit_price.to_pesos(), line.discount.to_pesos(),
                    line.line_subtotal.to_pesos(), line.unit_cost.to_pesos(),
                    line.variant_id, line.variant_label
                ],
            ).map_err(|e| e.to_string())?;

            // La existencia se lee aquí y se descuenta de forma relativa. Con
            // una foto tomada al armar los renglones, dos tallas del mismo
            // vestido partían las dos del mismo número y la segunda deshacía el
            // descuento de la primera.
            let prev_stock: i32 = db
                .query_row(
                    "SELECT stock FROM products WHERE id = ?1",
                    params![line.product_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let new_stock = prev_stock - line.quantity;

            db.execute(
                "UPDATE products SET stock = stock - ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![line.quantity, line.product_id],
            ).map_err(|e| e.to_string())?;

            // Variant-level stock, when applicable.
            if let Some(vid) = line.variant_id {
                db.execute(
                    "UPDATE product_variants SET stock = stock - ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![line.quantity, vid],
                ).map_err(|e| e.to_string())?;
            }

            let reason = line.variant_label.as_ref().map(|l| format!("Venta ({})", l));
            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'sale', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    line.product_id, -line.quantity, prev_stock,
                    new_stock, sale_id, reason, user_id
                ],
            ).map_err(|e| e.to_string())?;
        }

        // Persist the breakdown and move each leg into its register column.
        for (method, amount) in &applied {
            db.execute(
                "INSERT INTO sale_payments (sale_id, method, amount) VALUES (?1, ?2, ?3)",
                params![sale_id, method, amount.to_pesos()],
            ).map_err(|e| e.to_string())?;

            db.execute(
                &format!(
                    "UPDATE cash_registers SET {} = {} + ?1 WHERE id = ?2",
                    register_field(method), register_field(method)
                ),
                params![amount.to_pesos(), register_id],
            ).map_err(|e| e.to_string())?;
        }
        db.execute(
            "UPDATE cash_registers SET total_sales = total_sales + ?1, sale_count = sale_count + 1 WHERE id = ?2",
            params![total.to_pesos(), register_id],
        ).map_err(|e| e.to_string())?;

        // Best-effort audit log
        db.execute(
            "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'sales', ?1, ?2)",
            params![format!("Venta {} registrada por {}", folio, total), user_id],
        ).ok();

        // Return the created sale
        get_sale_by_id(db, sale_id)
    })();

    match result {
        Ok(sale) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(sale)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn cancel_sale(state: State<DbState>, sessions: State<SessionState>, token: String, sale_id: i64) -> Result<(), String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.conn();
    cancelar_venta(&db, user_id, sale_id)
}

/// Núcleo de la cancelación, con la conexión explícita.
pub fn cancelar_venta(db: &rusqlite::Connection, user_id: i64, sale_id: i64) -> Result<(), String> {

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        // Get sale status
        let status: String = db.query_row(
            "SELECT status FROM sales WHERE id = ?1",
            params![sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if status != "completed" {
            return Err("Solo se pueden cancelar ventas completadas".to_string());
        }

        // Get sale items to restore stock
        let mut stmt = db.prepare(
            "SELECT product_id, quantity, variant_id FROM sale_items WHERE sale_id = ?1"
        ).map_err(|e| e.to_string())?;

        let items: Vec<(i64, i32, Option<i64>)> = stmt.query_map(params![sale_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        // Restore stock for each item
        for (product_id, quantity, variant_id) in items {
            let current_stock: i32 = db.query_row(
                "SELECT stock FROM products WHERE id = ?1",
                params![product_id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;

            let new_stock = current_stock + quantity;

            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, product_id],
            ).map_err(|e| e.to_string())?;

            if let Some(vid) = variant_id {
                db.execute(
                    "UPDATE product_variants SET stock = stock + ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![quantity, vid],
                ).map_err(|e| e.to_string())?;
            }

            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'cancellation', ?2, ?3, ?4, ?5, 'Cancelación de venta', ?6)",
                params![product_id, quantity, current_stock, new_stock, sale_id, user_id],
            ).map_err(|e| e.to_string())?;
        }

        // Update sale status
        db.execute(
            "UPDATE sales SET status = 'cancelled' WHERE id = ?1",
            params![sale_id],
        ).map_err(|e| e.to_string())?;

        // Reverse cash register totals ONLY if that register is still open.
        // Touching a closed shift would desync its already-computed cut.
        let (total, cr_id): (f64, Option<i64>) = db.query_row(
            "SELECT total, cash_register_id FROM sales WHERE id = ?1",
            params![sale_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| e.to_string())?;

        if let Some(cr_id) = cr_id {
            let is_open: bool = db.query_row(
                "SELECT status = 'open' FROM cash_registers WHERE id = ?1",
                params![cr_id],
                |row| row.get(0),
            ).unwrap_or(false);

            if is_open {
                // Reverse each leg of the tender into the column it landed in.
                let mut stmt = db.prepare(
                    "SELECT method, amount FROM sale_payments WHERE sale_id = ?1"
                ).map_err(|e| e.to_string())?;
                let legs: Vec<(String, f64)> = stmt
                    .query_map(params![sale_id], |row| Ok((row.get(0)?, row.get(1)?)))
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                drop(stmt);

                for (method, amount) in legs {
                    db.execute(
                        &format!(
                            "UPDATE cash_registers SET {} = {} - ?1 WHERE id = ?2",
                            register_field(&method), register_field(&method)
                        ),
                        params![amount, cr_id],
                    ).map_err(|e| e.to_string())?;
                }

                db.execute(
                    "UPDATE cash_registers SET total_sales = total_sales - ?1, sale_count = sale_count - 1 WHERE id = ?2",
                    params![total, cr_id],
                ).map_err(|e| e.to_string())?;
            }
        }

        db.execute(
            "INSERT INTO app_logs (level, module, message, user_id) VALUES ('warn', 'sales', ?1, ?2)",
            params![format!("Venta {} cancelada", sale_id), user_id],
        ).ok();

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn get_sales(state: State<DbState>, sessions: State<SessionState>, token: String, filters: Option<SaleFilters>) -> Result<Vec<Sale>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    let filters = filters.unwrap_or_default();

    let mut sql = String::from(
        "SELECT s.id, s.folio, s.user_id, s.cash_register_id, s.subtotal, s.discount_total, s.tax, s.total,
                s.payment_method, s.amount_paid, s.change_amount, s.status, s.notes, s.created_at,
                s.customer_id, u.full_name AS user_name, c.name AS customer_name
         FROM sales s
         LEFT JOIN users u ON s.user_id = u.id
         LEFT JOIN customers c ON s.customer_id = c.id
         WHERE 1=1"
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref from) = filters.date_from {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND date(s.created_at) >= date(?{})", idx));
        param_values.push(Box::new(from.clone()));
    }
    if let Some(ref to) = filters.date_to {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND date(s.created_at) <= date(?{})", idx));
        param_values.push(Box::new(to.clone()));
    }
    if let Some(ref status) = filters.status {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.status = ?{}", idx));
        param_values.push(Box::new(status.clone()));
    }
    if let Some(ref pm) = filters.payment_method {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.payment_method = ?{}", idx));
        param_values.push(Box::new(pm.clone()));
    }
    if let Some(uid) = filters.user_id {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.user_id = ?{}", idx));
        param_values.push(Box::new(uid));
    }

    sql.push_str(" ORDER BY s.created_at DESC LIMIT 1000");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let sales = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(Sale {
                id: row.get(0)?,
                folio: row.get(1)?,
                user_id: row.get(2)?,
                cash_register_id: row.get(3)?,
                subtotal: row.get(4)?,
                discount_total: row.get(5)?,
                tax: row.get(6)?,
                total: row.get(7)?,
                payment_method: row.get(8)?,
                amount_paid: row.get(9)?,
                change_amount: row.get(10)?,
                status: row.get(11)?,
                notes: row.get(12)?,
                created_at: row.get(13)?,
                customer_id: row.get(14)?,
                user_name: row.get(15)?,
                customer_name: row.get(16)?,
                items: None,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(sales)
}

#[tauri::command]
pub fn get_sale_detail(state: State<DbState>, sessions: State<SessionState>, token: String, sale_id: i64) -> Result<Sale, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    get_sale_by_id(&db, sale_id)
}

fn get_sale_by_id(db: &rusqlite::Connection, sale_id: i64) -> Result<Sale, String> {
    let mut sale = db.query_row(
        "SELECT s.id, s.folio, s.user_id, s.cash_register_id, s.subtotal, s.discount_total, s.tax, s.total,
                s.payment_method, s.amount_paid, s.change_amount, s.status, s.notes, s.created_at,
                s.customer_id, u.full_name AS user_name, c.name AS customer_name
         FROM sales s
         LEFT JOIN users u ON s.user_id = u.id
         LEFT JOIN customers c ON s.customer_id = c.id
         WHERE s.id = ?1",
        params![sale_id],
        |row| {
            Ok(Sale {
                id: row.get(0)?,
                folio: row.get(1)?,
                user_id: row.get(2)?,
                cash_register_id: row.get(3)?,
                subtotal: row.get(4)?,
                discount_total: row.get(5)?,
                tax: row.get(6)?,
                total: row.get(7)?,
                payment_method: row.get(8)?,
                amount_paid: row.get(9)?,
                change_amount: row.get(10)?,
                status: row.get(11)?,
                notes: row.get(12)?,
                created_at: row.get(13)?,
                customer_id: row.get(14)?,
                user_name: row.get(15)?,
                customer_name: row.get(16)?,
                items: None,
            })
        },
    ).map_err(|e| e.to_string())?;

    // Get items
    let mut stmt = db.prepare(
        "SELECT id, sale_id, product_id, product_name, product_sku, quantity, unit_price, discount, subtotal, returned_quantity, variant_id, variant_label FROM sale_items WHERE sale_id = ?1"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(params![sale_id], |row| {
        Ok(SaleItem {
            id: row.get(0)?,
            sale_id: row.get(1)?,
            product_id: row.get(2)?,
            product_name: row.get(3)?,
            product_sku: row.get(4)?,
            quantity: row.get(5)?,
            unit_price: row.get(6)?,
            discount: row.get(7)?,
            subtotal: row.get(8)?,
            returned_quantity: row.get(9)?,
            variant_id: row.get(10)?,
            variant_label: row.get(11)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    sale.items = Some(items);
    Ok(sale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_label_joins_present_attributes() {
        let s = |v: &str| Some(v.to_string());
        assert_eq!(variant_label(&s("M"), &s("Negro")), "M / Negro");
        assert_eq!(variant_label(&s("M"), &None), "M");
        assert_eq!(variant_label(&None, &s("Negro")), "Negro");
        assert_eq!(variant_label(&None, &None), "Único");
        assert_eq!(variant_label(&s("  "), &s("Rojo")), "Rojo");
    }

    fn split(method: &str, amount: f64) -> PaymentSplitDto {
        PaymentSplitDto { method: method.to_string(), amount }
    }

    fn pesos(v: f64) -> Cents {
        Cents::from_pesos(v)
    }

    fn leg(applied: &[(String, Cents)], method: &str) -> Cents {
        applied.iter().find(|(m, _)| m == method).map(|(_, a)| *a).unwrap_or(Cents::ZERO)
    }

    #[test]
    fn cash_only_tender_returns_the_change() {
        let (applied, change) = split_tender(&[split("cash", 500.0)], pesos(249.0)).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].1, pesos(249.0));
        assert_eq!(change, pesos(251.0));
    }

    #[test]
    fn mixed_tender_applies_cash_to_the_remainder() {
        let (applied, change) =
            split_tender(&[split("card", 200.0), split("cash", 100.0)], pesos(249.0)).unwrap();

        assert_eq!(leg(&applied, "card"), pesos(200.0));
        assert_eq!(leg(&applied, "cash"), pesos(49.0));
        assert_eq!(change, pesos(51.0));
        assert_eq!(leg(&applied, "card") + leg(&applied, "cash"), pesos(249.0));
    }

    #[test]
    fn insufficient_tender_is_rejected() {
        assert!(split_tender(&[split("cash", 100.0)], pesos(249.0)).is_err());
        assert!(split_tender(&[split("card", 100.0), split("cash", 40.0)], pesos(249.0)).is_err());
    }

    #[test]
    fn non_cash_cannot_exceed_the_total() {
        assert!(split_tender(&[split("card", 300.0)], pesos(249.0)).is_err());
    }

    #[test]
    fn a_negative_leg_is_rejected() {
        assert!(split_tender(&[split("card", -50.0), split("cash", 300.0)], pesos(249.0)).is_err());
    }

    #[test]
    fn a_zero_tender_against_a_real_total_is_insufficient() {
        let err = split_tender(&[split("cash", 0.0)], pesos(249.0)).unwrap_err();
        assert!(err.contains("insuficiente"), "mensaje poco claro: {}", err);
    }

    #[test]
    fn a_sale_that_adds_up_to_zero_needs_no_tender() {
        let (applied, change) = split_tender(&[split("cash", 0.0)], Cents::ZERO).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].1, Cents::ZERO);
        assert_eq!(change, Cents::ZERO);
    }

    #[test]
    fn card_only_tender_leaves_no_change() {
        let (applied, change) = split_tender(&[split("card", 249.0)], pesos(249.0)).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(change, Cents::ZERO);
    }

    #[test]
    fn a_tender_paid_to_the_exact_cent_leaves_no_change() {
        // Con f64 este caso podía dejar un centavo fantasma de cambio.
        let (applied, change) = split_tender(
            &[split("card", 0.1), split("card", 0.2), split("cash", 0.3)],
            pesos(0.6),
        ).unwrap();
        assert_eq!(change, Cents::ZERO);
        assert_eq!(applied.iter().map(|(_, a)| *a).sum::<Cents>(), pesos(0.6));
    }

    #[test]
    fn register_field_maps_every_method() {
        assert_eq!(register_field("cash"), "total_cash_sales");
        assert_eq!(register_field("card"), "total_card_sales");
        assert_eq!(register_field("transfer"), "total_transfer_sales");
        assert_eq!(register_field("desconocido"), "total_cash_sales");
    }

    fn db_with_promo(
        discount_type: &str,
        value: f64,
        applies_to: &str,
        target_id: Option<i64>,
        active: bool,
        days_offset: i64,
    ) -> (rusqlite::Connection, i64) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id)
             VALUES ('Promo', ?1, ?2, date('now','localtime', ?3 || ' days'), date('now','localtime', ?4 || ' days'), ?5, ?6, ?7)",
            params![
                discount_type, value,
                (days_offset - 1).to_string(), (days_offset + 1).to_string(),
                active as i32, applies_to, target_id
            ],
        ).unwrap();
        let id = conn.last_insert_rowid();
        (conn, id)
    }

    /// (product_id, category_id, importe neto de la línea)
    fn basket() -> Vec<(i64, Option<i64>, Cents)> {
        vec![(1, Some(10), pesos(200.0)), (2, Some(20), pesos(300.0))]
    }

    #[test]
    fn a_percentage_promotion_applies_to_the_whole_basket() {
        let (conn, id) = db_with_promo("percentage", 10.0, "all", None, true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(50.0));
    }

    #[test]
    fn a_category_promotion_only_touches_its_category() {
        let (conn, id) = db_with_promo("percentage", 50.0, "category", Some(10), true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(100.0));
    }

    #[test]
    fn a_product_promotion_only_touches_its_product() {
        let (conn, id) = db_with_promo("percentage", 10.0, "product", Some(2), true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(30.0));
    }

    #[test]
    fn a_fixed_promotion_never_exceeds_the_basket() {
        let (conn, id) = db_with_promo("fixed", 9999.0, "all", None, true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(500.0));
    }

    #[test]
    fn an_expired_promotion_grants_nothing() {
        let (conn, id) = db_with_promo("percentage", 50.0, "all", None, true, -30);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(0.0));
    }

    #[test]
    fn a_future_promotion_grants_nothing() {
        let (conn, id) = db_with_promo("percentage", 50.0, "all", None, true, 30);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(0.0));
    }

    #[test]
    fn a_deactivated_promotion_grants_nothing() {
        let (conn, id) = db_with_promo("percentage", 50.0, "all", None, false, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(0.0));
    }

    #[test]
    fn an_invented_promotion_id_grants_nothing() {
        let (conn, _) = db_with_promo("percentage", 50.0, "all", None, true, 0);
        assert_eq!(promotion_discount(&conn, 9999, &basket()).unwrap(), pesos(0.0));
    }

    #[test]
    fn a_percentage_over_100_cannot_pay_the_customer() {
        let (conn, id) = db_with_promo("percentage", 500.0, "all", None, true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(500.0));
    }

    #[test]
    fn a_promotion_whose_category_is_absent_grants_nothing() {
        let (conn, id) = db_with_promo("percentage", 50.0, "category", Some(99), true, 0);
        assert_eq!(promotion_discount(&conn, id, &basket()).unwrap(), pesos(0.0));
    }

    fn db_ventas() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'x', 'x', 'X', 'admin')",
            [],
        ).unwrap();
        conn
    }

    fn insertar_folio(conn: &rusqlite::Connection, folio: &str) {
        conn.execute(
            "INSERT INTO sales (folio, user_id, subtotal, discount_total, tax, total,
                                payment_method, amount_paid, change_amount)
             VALUES (?1, 1, 0, 0, 0, 0, 'cash', 0, 0)",
            params![folio],
        ).unwrap();
    }

    fn hoy() -> String {
        chrono::Local::now().format("%Y%m%d").to_string()
    }

    #[test]
    fn the_first_sale_of_the_day_starts_at_one() {
        let conn = db_ventas();
        assert_eq!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap(), format!("V-{}-001", hoy()));
    }

    #[test]
    fn folios_continue_from_the_highest_already_issued() {
        let conn = db_ventas();
        insertar_folio(&conn, &format!("V-{}-001", hoy()));
        insertar_folio(&conn, &format!("V-{}-002", hoy()));
        assert_eq!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap(), format!("V-{}-003", hoy()));
    }

    #[test]
    fn a_missing_row_does_not_make_the_folio_repeat() {
        // Con COUNT(*) este caso devolvía 003, que ya existía, y la venta
        // fallaba por folio duplicado.
        let conn = db_ventas();
        insertar_folio(&conn, &format!("V-{}-001", hoy()));
        insertar_folio(&conn, &format!("V-{}-002", hoy()));
        insertar_folio(&conn, &format!("V-{}-003", hoy()));
        conn.execute("DELETE FROM sales WHERE folio LIKE '%-002'", []).unwrap();

        assert_eq!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap(), format!("V-{}-004", hoy()));
    }

    #[test]
    fn folios_from_other_days_do_not_interfere() {
        let conn = db_ventas();
        insertar_folio(&conn, "V-20200101-999");
        assert_eq!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap(), format!("V-{}-001", hoy()));
    }

    #[test]
    fn a_second_terminal_issues_its_own_series() {
        let conn = db_ventas();
        insertar_folio(&conn, &format!("V-{}-001", hoy()));

        conn.execute("UPDATE system_config SET value = '02' WHERE key = 'terminal_id'", []).unwrap();

        // La caja 2 no continúa la serie de la caja 1: emite la suya.
        assert_eq!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap(), format!("V02-{}-001", hoy()));
    }

    #[test]
    fn the_default_terminal_keeps_the_plain_folio_format() {
        let conn = db_ventas();
        assert!(crate::folios::siguiente(&conn, crate::folios::Serie::Ventas).unwrap().starts_with("V-"));
        assert_eq!(crate::folios::terminal_id(&conn), "01");
    }

    #[test]
    fn config_number_falls_back_on_missing_or_invalid_values() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();

        assert_eq!(config_number(&conn, "no_existe", 7.5), 7.5);
        assert_eq!(config_number(&conn, "tax_rate", 99.0), 0.0);

        conn.execute("UPDATE system_config SET value = 'x' WHERE key = 'tax_rate'", [])
            .unwrap();
        assert_eq!(config_number(&conn, "tax_rate", 16.0), 16.0);
    }
}

/// Pruebas del flujo completo de cobro contra una base real.
///
/// Las pruebas unitarias cubren cada pieza por separado; estas verifican que
/// juntas producen los números correctos y dejan la base consistente, que es lo
/// que de verdad importa cuando hay dinero de por medio.
#[cfg(test)]
mod integracion {
    use super::*;
    use crate::models::sale::CreateSaleItemDto;
    use crate::commands::cash_register::{expected_cash, open_register_id};

    struct Tienda {
        db: rusqlite::Connection,
    }

    impl Tienda {
        fn nueva() -> Tienda {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::db::migrations::run_migrations(&db).unwrap();
            db.execute(
                "INSERT INTO users (id, username, password_hash, full_name, role)
                 VALUES (1, 'cajero', 'x', 'Cajero', 'cashier')",
                [],
            ).unwrap();
            Tienda { db }
        }

        fn con_caja(self, fondo: f64) -> Tienda {
            self.db.execute(
                "INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, ?1)",
                params![fondo],
            ).unwrap();
            self
        }

        fn producto(&self, sku: &str, precio: f64, costo: f64, stock: i32) -> i64 {
            self.db.execute(
                "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
                 VALUES (?1, ?1, ?2, ?3, ?4)",
                params![sku, costo, precio, stock],
            ).unwrap();
            self.db.last_insert_rowid()
        }

        fn cobrar(&self, data: CreateSaleDto) -> Result<Sale, String> {
            registrar_venta(&self.db, 1, None, data)
        }

        fn stock(&self, product_id: i64) -> i32 {
            self.db.query_row("SELECT stock FROM products WHERE id = ?1", params![product_id], |r| r.get(0)).unwrap()
        }

        fn caja(&self) -> crate::models::cash_register::CashRegister {
            let id = open_register_id(&self.db).unwrap();
            self.db.query_row(
                "SELECT cr.id, cr.user_id, u.full_name, cr.opening_amount, cr.closing_amount,
                        cr.expected_amount, cr.difference, cr.total_sales, cr.total_cash_sales,
                        cr.total_card_sales, cr.total_transfer_sales, cr.total_layaway_cash,
                        cr.total_layaway_card, cr.total_layaway_transfer, cr.total_refunds_cash,
                        cr.total_expenses, cr.sale_count, cr.status, cr.opened_at, cr.closed_at
                 FROM cash_registers cr LEFT JOIN users u ON cr.user_id = u.id WHERE cr.id = ?1",
                params![id],
                |row| Ok(crate::models::cash_register::CashRegister {
                    id: row.get(0)?, user_id: row.get(1)?, user_name: row.get(2)?,
                    opening_amount: row.get(3)?, closing_amount: row.get(4)?,
                    expected_amount: row.get(5)?, difference: row.get(6)?,
                    total_sales: row.get(7)?, total_cash_sales: row.get(8)?,
                    total_card_sales: row.get(9)?, total_transfer_sales: row.get(10)?,
                    total_layaway_cash: row.get(11)?, total_layaway_card: row.get(12)?,
                    total_layaway_transfer: row.get(13)?, total_refunds_cash: row.get(14)?,
                    total_expenses: row.get(15)?, sale_count: row.get(16)?,
                    status: row.get(17)?, opened_at: row.get(18)?, closed_at: row.get(19)?,
                }),
            ).unwrap()
        }
    }

    #[test]
    fn dos_tallas_del_mismo_modelo_descuentan_las_dos() {
        // El caso normal de una tienda de ropa: la clienta se lleva el mismo
        // vestido en mediana y en grande. Son dos renglones del mismo producto.
        let t = Tienda::nueva().con_caja(500.0);
        let vestido = t.producto("VES", 499.0, 200.0, 0);
        t.db.execute(
            "INSERT INTO product_variants (id, product_id, size, stock) VALUES
             (1, ?1, 'M', 5), (2, ?1, 'G', 5)", params![vestido]).unwrap();
        t.db.execute("UPDATE products SET has_variants = 1, stock = 10 WHERE id = ?1",
                     params![vestido]).unwrap();

        let mut data = venta(vec![]);
        data.items = vec![
            CreateSaleItemDto { product_id: vestido, quantity: 2, unit_price: 0.0, discount: 0.0, variant_id: Some(1) },
            CreateSaleItemDto { product_id: vestido, quantity: 3, unit_price: 0.0, discount: 0.0, variant_id: Some(2) },
        ];
        t.cobrar(data).unwrap();

        let m: i32 = t.db.query_row("SELECT stock FROM product_variants WHERE id = 1", [], |r| r.get(0)).unwrap();
        let g: i32 = t.db.query_row("SELECT stock FROM product_variants WHERE id = 2", [], |r| r.get(0)).unwrap();
        assert_eq!((m, g), (3, 2), "cada talla descuenta lo suyo");
        assert_eq!(t.stock(vestido), 5, "el total del producto es la suma de sus tallas");
    }

    #[test]
    fn el_mismo_producto_en_dos_renglones_no_puede_rebasar_la_existencia() {
        let t = Tienda::nueva().con_caja(500.0);
        let blusa = t.producto("BLU", 299.0, 100.0, 5);

        let error = t.cobrar(venta(vec![(blusa, 3), (blusa, 3)]));

        assert!(error.is_err(), "seis piezas de cinco no deberían venderse");
        assert_eq!(t.stock(blusa), 5, "una venta rechazada no toca el inventario");
    }

    fn venta(items: Vec<(i64, i32)>) -> CreateSaleDto {
        CreateSaleDto {
            items: items.into_iter().map(|(product_id, quantity)| CreateSaleItemDto {
                product_id, quantity, unit_price: 0.0, discount: 0.0, variant_id: None,
            }).collect(),
            payment_method: "cash".to_string(),
            amount_paid: 100_000.0,
            payments: vec![],
            discount_total: 0.0,
            promotion_id: None,
            requiere_factura: false,
            notes: None,
            customer_id: None,
            client_request_id: None,
        }
    }

    #[test]
    fn una_venta_simple_cuadra_de_punta_a_punta() {
        let t = Tienda::nueva().con_caja(1000.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let sale = t.cobrar(venta(vec![(p, 2)])).unwrap();

        assert_eq!(sale.total, 498.0);
        assert_eq!(sale.change_amount, 100_000.0 - 498.0);
        assert_eq!(t.stock(p), 8, "el stock debe bajar");

        let caja = t.caja();
        assert_eq!(caja.total_cash_sales, 498.0);
        assert_eq!(caja.sale_count, 1);
        assert_eq!(expected_cash(&caja), 1498.0);
    }

    #[test]
    fn el_precio_sale_de_la_base_aunque_el_cliente_mienta() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.items[0].unit_price = 1.0; // el cliente intenta cobrar $1

        assert_eq!(t.cobrar(data).unwrap().total, 249.0);
    }

    #[test]
    fn el_descuento_del_cliente_se_ignora() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.discount_total = 248.0; // intento de dejar la venta en $1

        assert_eq!(t.cobrar(data).unwrap().total, 249.0);
    }

    #[test]
    fn un_descuento_de_linea_no_puede_superar_la_linea() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.items[0].discount = 9999.0;

        let sale = t.cobrar(data).unwrap();
        assert_eq!(sale.total, 0.0, "el descuento se topa, no deja el total negativo");
        assert_eq!(sale.discount_total, 249.0);
    }

    #[test]
    fn sin_caja_abierta_no_se_cobra() {
        let t = Tienda::nueva();
        let p = t.producto("CAM", 249.0, 100.0, 10);

        assert!(t.cobrar(venta(vec![(p, 1)])).is_err());
    }

    #[test]
    fn una_venta_sin_stock_se_rechaza_y_no_deja_rastro() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 1);

        assert!(t.cobrar(venta(vec![(p, 5)])).is_err());

        let ventas: i64 = t.db.query_row("SELECT COUNT(*) FROM sales", [], |r| r.get(0)).unwrap();
        assert_eq!(ventas, 0, "la transacción debe revertirse completa");
        assert_eq!(t.stock(p), 1, "el stock no debe moverse");
        assert_eq!(t.caja().sale_count, 0);
    }

    #[test]
    fn el_efectivo_insuficiente_se_rechaza() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.amount_paid = 100.0;

        assert!(t.cobrar(data).is_err());
    }

    #[test]
    fn el_mismo_cobro_enviado_dos_veces_produce_una_sola_venta() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.client_request_id = Some("intento-1".to_string());
        let primera = t.cobrar(data).unwrap();

        let mut repetida = venta(vec![(p, 1)]);
        repetida.client_request_id = Some("intento-1".to_string());
        let segunda = t.cobrar(repetida).unwrap();

        assert_eq!(primera.id, segunda.id, "debe devolver la venta original");
        assert_eq!(t.stock(p), 9, "el stock solo baja una vez");
        assert_eq!(t.caja().sale_count, 1);
    }

    #[test]
    fn el_pago_mixto_reparte_entre_metodos_y_el_cajon_solo_recibe_efectivo() {
        let t = Tienda::nueva().con_caja(1000.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.payments = vec![
            PaymentSplitDto { method: "card".into(), amount: 200.0 },
            PaymentSplitDto { method: "cash".into(), amount: 100.0 },
        ];
        let sale = t.cobrar(data).unwrap();

        assert_eq!(sale.payment_method, "mixed");
        assert_eq!(sale.change_amount, 51.0);

        let caja = t.caja();
        assert_eq!(caja.total_card_sales, 200.0);
        assert_eq!(caja.total_cash_sales, 49.0);
        assert_eq!(caja.total_sales, 249.0);
        // En el cajón entraron 49, no los 100 que dio el cliente.
        assert_eq!(expected_cash(&caja), 1049.0);
    }

    #[test]
    fn una_venta_con_tarjeta_no_toca_el_cajon() {
        let t = Tienda::nueva().con_caja(1000.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.payment_method = "card".to_string();
        data.amount_paid = 0.0;
        t.cobrar(data).unwrap();

        let caja = t.caja();
        assert_eq!(caja.total_card_sales, 249.0);
        assert_eq!(expected_cash(&caja), 1000.0);
    }

    #[test]
    fn la_promocion_se_aplica_con_el_valor_del_servidor() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 200.0, 100.0, 10);
        t.db.execute(
            "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, applies_to)
             VALUES ('10off', 'percentage', 10, date('now','localtime','-1 day'), date('now','localtime','+1 day'), 'all')",
            [],
        ).unwrap();
        let promo_id = t.db.last_insert_rowid();

        let mut data = venta(vec![(p, 1)]);
        data.promotion_id = Some(promo_id);
        data.discount_total = 190.0; // el cliente pide un descuento absurdo

        let sale = t.cobrar(data).unwrap();
        assert_eq!(sale.discount_total, 20.0, "manda el 10% de la promoción");
        assert_eq!(sale.total, 180.0);
    }

    #[test]
    fn una_promocion_vencida_no_descuenta_nada() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 200.0, 100.0, 10);
        t.db.execute(
            "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, applies_to)
             VALUES ('vieja', 'percentage', 50, '2020-01-01', '2020-01-31', 'all')",
            [],
        ).unwrap();
        let promo_id = t.db.last_insert_rowid();

        let mut data = venta(vec![(p, 1)]);
        data.promotion_id = Some(promo_id);

        assert_eq!(t.cobrar(data).unwrap().total, 200.0);
    }

    #[test]
    fn el_impuesto_se_calcula_sobre_la_base_ya_descontada() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 100.0, 50.0, 10);
        t.db.execute("UPDATE system_config SET value = '16' WHERE key = 'tax_rate'", []).unwrap();

        let mut data = venta(vec![(p, 1)]);
        data.items[0].discount = 20.0;

        let sale = t.cobrar(data).unwrap();
        assert_eq!(sale.subtotal, 100.0);
        assert_eq!(sale.discount_total, 20.0);
        assert_eq!(sale.tax, 12.80, "16% de 80");
        assert_eq!(sale.total, 92.80);
    }

    #[test]
    fn las_partidas_guardadas_cuadran_con_el_encabezado() {
        let t = Tienda::nueva().con_caja(0.0);
        let a = t.producto("A", 100.0, 50.0, 10);
        let b = t.producto("B", 250.0, 90.0, 10);

        let mut data = venta(vec![(a, 2), (b, 1)]);
        data.items[0].discount = 15.0;
        let sale = t.cobrar(data).unwrap();

        let suma_partidas: f64 = t.db.query_row(
            "SELECT SUM(subtotal) FROM sale_items WHERE sale_id = ?1", params![sale.id], |r| r.get(0),
        ).unwrap();

        // subtotal bruto - descuentos de línea == suma de las partidas
        assert_eq!(suma_partidas, sale.subtotal - sale.discount_total);
        assert_eq!(sale.total, suma_partidas + sale.tax);
    }

    #[test]
    fn el_desglose_del_cobro_suma_exactamente_el_total() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 333.33, 100.0, 10);

        let mut data = venta(vec![(p, 3)]);
        data.payments = vec![
            PaymentSplitDto { method: "transfer".into(), amount: 500.0 },
            PaymentSplitDto { method: "cash".into(), amount: 600.0 },
        ];
        let sale = t.cobrar(data).unwrap();

        let suma: f64 = t.db.query_row(
            "SELECT SUM(amount) FROM sale_payments WHERE sale_id = ?1", params![sale.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(suma, sale.total);
    }

    #[test]
    fn facturar_sin_cliente_se_rechaza_y_revierte() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.requiere_factura = true;

        assert!(t.cobrar(data).is_err());
        let ventas: i64 = t.db.query_row("SELECT COUNT(*) FROM sales", [], |r| r.get(0)).unwrap();
        assert_eq!(ventas, 0);
        assert_eq!(t.stock(p), 10);
    }

    #[test]
    fn facturar_con_cliente_sin_rfc_se_rechaza() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);
        t.db.execute("INSERT INTO customers (id, name) VALUES (7, 'Sin RFC')", []).unwrap();

        let mut data = venta(vec![(p, 1)]);
        data.requiere_factura = true;
        data.customer_id = Some(7);

        let err = t.cobrar(data).unwrap_err();
        assert!(err.contains("RFC"), "el mensaje debe decir qué falta: {}", err);
    }

    #[test]
    fn facturar_guarda_una_copia_de_los_datos_fiscales() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);
        t.db.execute(
            "INSERT INTO customers (id, name, rfc, razon_social, regimen_fiscal, cp_fiscal, uso_cfdi)
             VALUES (7, 'Cliente', 'ABC010101AB1', 'Ropa SA', '601', '64000', 'G03')",
            [],
        ).unwrap();

        let mut data = venta(vec![(p, 1)]);
        data.requiere_factura = true;
        data.customer_id = Some(7);
        let sale = t.cobrar(data).unwrap();

        let (rfc, razon, regimen): (String, String, String) = t.db.query_row(
            "SELECT fiscal_rfc, fiscal_razon_social, fiscal_regimen FROM sales WHERE id = ?1",
            params![sale.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();

        assert_eq!(rfc, "ABC010101AB1");
        assert_eq!(razon, "Ropa SA");
        assert_eq!(regimen, "601");

        // Si el cliente cambia su RFC después, la venta conserva el de entonces.
        t.db.execute("UPDATE customers SET rfc = 'XXX999999XX9' WHERE id = 7", []).unwrap();
        let guardado: String = t.db.query_row(
            "SELECT fiscal_rfc FROM sales WHERE id = ?1", params![sale.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(guardado, "ABC010101AB1");
    }

    #[test]
    fn una_venta_que_suma_cero_no_revienta() {
        // Un obsequio o una línea totalmente descontada dejaba el desglose vacío
        // y el backend se caía al leer la primera forma de pago.
        let t = Tienda::nueva().con_caja(500.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);

        let mut data = venta(vec![(p, 1)]);
        data.items[0].discount = 249.0;
        data.amount_paid = 0.0;

        let sale = t.cobrar(data).unwrap();
        assert_eq!(sale.total, 0.0);
        assert_eq!(sale.payment_method, "cash");
        assert_eq!(t.stock(p), 9, "el producto sí sale del inventario");
        assert_eq!(expected_cash(&t.caja()), 500.0, "no entra dinero al cajón");
    }

    #[test]
    fn los_folios_del_dia_avanzan_sin_repetirse() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 10.0, 5.0, 100);

        let folios: Vec<String> = (0..5)
            .map(|_| t.cobrar(venta(vec![(p, 1)])).unwrap().folio)
            .collect();

        let unicos: std::collections::HashSet<_> = folios.iter().collect();
        assert_eq!(unicos.len(), 5, "no debe haber folios repetidos: {:?}", folios);
        assert!(folios[4].ends_with("005"));
    }

    #[test]
    fn el_costo_queda_congelado_en_la_partida() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);
        let sale = t.cobrar(venta(vec![(p, 1)])).unwrap();

        // Resurtir más caro no debe reescribir la utilidad de una venta pasada.
        t.db.execute("UPDATE products SET purchase_price = 180.0 WHERE id = ?1", params![p]).unwrap();

        let costo: f64 = t.db.query_row(
            "SELECT unit_cost FROM sale_items WHERE sale_id = ?1", params![sale.id], |r| r.get(0),
        ).unwrap();
        assert_eq!(costo, 100.0);
    }

    #[test]
    fn se_registra_el_movimiento_de_inventario() {
        let t = Tienda::nueva().con_caja(0.0);
        let p = t.producto("CAM", 249.0, 100.0, 10);
        let sale = t.cobrar(venta(vec![(p, 3)])).unwrap();

        let (tipo, cantidad, previo, nuevo): (String, i32, i32, i32) = t.db.query_row(
            "SELECT movement_type, quantity, previous_stock, new_stock
             FROM inventory_movements WHERE reference_id = ?1",
            params![sale.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap();

        assert_eq!(tipo, "sale");
        assert_eq!(cantidad, -3);
        assert_eq!(previo, 10);
        assert_eq!(nuevo, 7);
    }
}
