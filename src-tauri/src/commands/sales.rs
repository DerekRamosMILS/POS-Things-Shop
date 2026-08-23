use rusqlite::params;
use tauri::State;
use chrono::Local;

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

/// Terminal desde la que se cobra. Con una sola caja siempre es "01".
fn terminal_id(db: &rusqlite::Connection) -> String {
    db.query_row(
        "SELECT value FROM system_config WHERE key = 'terminal_id'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .unwrap_or_else(|| "01".to_string())
}

/// Siguiente folio del día.
///
/// Se calcula desde el consecutivo más alto ya emitido, no contando renglones:
/// contar produce folios repetidos en cuanto falta una fila, y `folio` es único,
/// así que la venta fallaría al cobrar. Con más de una terminal el folio lleva
/// además su identificador, para que dos cajas no emitan el mismo número.
fn next_folio(db: &rusqlite::Connection) -> Result<String, String> {
    let today = Local::now().format("%Y%m%d").to_string();
    let terminal = terminal_id(db);

    let prefix = if terminal == "01" {
        format!("V-{}-", today)
    } else {
        format!("V{}-{}-", terminal, today)
    };

    // El consecutivo son los caracteres que siguen al prefijo.
    let last: i64 = db
        .query_row(
            "SELECT COALESCE(MAX(CAST(substr(folio, ?2) AS INTEGER)), 0)
             FROM sales WHERE folio LIKE ?1",
            params![format!("{}%", prefix), prefix.len() as i64 + 1],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(format!("{}{:03}", prefix, last + 1))
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
        if !amount.is_positive() {
            return Err("Cada forma de pago debe ser mayor a cero".to_string());
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
    let db = state.db.lock().map_err(|e| e.to_string())?;

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
            return get_sale_by_id(&db, existing_id);
        }
    }

    // Begin atomic transaction
    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<Sale, String> {
        // A sale requires an OPEN cash register. Derive it from the DB instead of
        // trusting the (possibly stale) id persisted in the client.
        let register_id: i64 = match crate::commands::cash_register::open_register_id(&db) {
            Some(id) => id,
            None => return Err("La caja no está abierta. Ábrela antes de cobrar.".to_string()),
        };
        if let Some(cr) = cash_register_id {
            if cr != register_id {
                return Err("La caja indicada no coincide con la caja abierta.".to_string());
            }
        }

        let folio = next_folio(&db)?;

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
            prev_stock: i32,
            variant_id: Option<i64>,
            variant_label: Option<String>,
        }
        let mut lines: Vec<Line> = Vec::with_capacity(data.items.len());
        let mut subtotal = Cents::ZERO; // bruto, antes de cualquier descuento

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
                (Some(vid), Some(variant_label(&size, &color)), vstock)
            } else {
                (None, None, product_stock)
            };

            if available < item.quantity {
                let label = variant_label.clone().map(|l| format!(" ({})", l)).unwrap_or_default();
                return Err(format!(
                    "Stock insuficiente para '{}{}'. Disponible: {}, Solicitado: {}",
                    name, label, available, item.quantity
                ));
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
                prev_stock: product_stock,
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
            Some(id) => promotion_discount(&db, id, &promo_base)?,
            None => Cents::ZERO,
        };

        let discount_total = (line_discount_total + promo_discount).min(subtotal);
        let taxable = (subtotal - discount_total).clamp_non_negative();
        let tax_rate = config_number(&db, "tax_rate", 0.0);
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
                data.client_request_id, data.promotion_id, terminal_id(&db)
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

            let new_stock = line.prev_stock - line.quantity;

            // Product-level aggregate stock always decreases.
            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, line.product_id],
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
                    line.product_id, -line.quantity, line.prev_stock,
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
        get_sale_by_id(&db, sale_id)
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
    let db = state.db.lock().map_err(|e| e.to_string())?;

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
    let db = state.db.lock().map_err(|e| e.to_string())?;
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
    let db = state.db.lock().map_err(|e| e.to_string())?;
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
    fn zero_or_negative_legs_are_rejected() {
        assert!(split_tender(&[split("cash", 0.0)], pesos(249.0)).is_err());
        assert!(split_tender(&[split("card", -50.0), split("cash", 300.0)], pesos(249.0)).is_err());
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
        Local::now().format("%Y%m%d").to_string()
    }

    #[test]
    fn the_first_sale_of_the_day_starts_at_one() {
        let conn = db_ventas();
        assert_eq!(next_folio(&conn).unwrap(), format!("V-{}-001", hoy()));
    }

    #[test]
    fn folios_continue_from_the_highest_already_issued() {
        let conn = db_ventas();
        insertar_folio(&conn, &format!("V-{}-001", hoy()));
        insertar_folio(&conn, &format!("V-{}-002", hoy()));
        assert_eq!(next_folio(&conn).unwrap(), format!("V-{}-003", hoy()));
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

        assert_eq!(next_folio(&conn).unwrap(), format!("V-{}-004", hoy()));
    }

    #[test]
    fn folios_from_other_days_do_not_interfere() {
        let conn = db_ventas();
        insertar_folio(&conn, "V-20200101-999");
        assert_eq!(next_folio(&conn).unwrap(), format!("V-{}-001", hoy()));
    }

    #[test]
    fn a_second_terminal_issues_its_own_series() {
        let conn = db_ventas();
        insertar_folio(&conn, &format!("V-{}-001", hoy()));

        conn.execute("UPDATE system_config SET value = '02' WHERE key = 'terminal_id'", []).unwrap();

        // La caja 2 no continúa la serie de la caja 1: emite la suya.
        assert_eq!(next_folio(&conn).unwrap(), format!("V02-{}-001", hoy()));
    }

    #[test]
    fn the_default_terminal_keeps_the_plain_folio_format() {
        let conn = db_ventas();
        assert!(next_folio(&conn).unwrap().starts_with("V-"));
        assert_eq!(terminal_id(&conn), "01");
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
