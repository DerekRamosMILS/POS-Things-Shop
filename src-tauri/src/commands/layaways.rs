use std::collections::HashMap;

use rusqlite::params;
use tauri::State;

use crate::commands::cash_register::open_register_id;
use crate::money::Cents;
use crate::db::connection::DbState;
use crate::session::{require_admin, require_auth, SessionState};
use crate::models::layaway::{
    CreateLayawayDto, Layaway, LayawayItem, LayawayPayment,
};

fn variant_label(size: &Option<String>, color: &Option<String>) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(s) = size.as_deref() { if !s.trim().is_empty() { parts.push(s); } }
    if let Some(c) = color.as_deref() { if !c.trim().is_empty() { parts.push(c); } }
    if parts.is_empty() { "Único".to_string() } else { parts.join(" / ") }
}

const SEL: &str = "SELECT l.id, l.folio, l.customer_id, cu.name AS customer_name, l.user_id, u.full_name AS user_name,
    l.total, l.paid, l.status, l.notes, l.due_date, l.created_at, l.completed_at
    FROM layaways l
    LEFT JOIN customers cu ON l.customer_id = cu.id
    LEFT JOIN users u ON l.user_id = u.id";

fn row_to_layaway(row: &rusqlite::Row) -> rusqlite::Result<Layaway> {
    Ok(Layaway {
        id: row.get(0)?,
        folio: row.get(1)?,
        customer_id: row.get(2)?,
        customer_name: row.get(3)?,
        user_id: row.get(4)?,
        user_name: row.get(5)?,
        total: row.get(6)?,
        paid: row.get(7)?,
        status: row.get(8)?,
        notes: row.get(9)?,
        due_date: row.get(10)?,
        created_at: row.get(11)?,
        completed_at: row.get(12)?,
        items: None,
        payments: None,
    })
}

/// Column on `cash_registers` that accumulates layaway deposits per method.
fn layaway_register_field(method: &str) -> &'static str {
    match method {
        "card" => "total_layaway_card",
        "transfer" => "total_layaway_transfer",
        _ => "total_layaway_cash",
    }
}

/// Record a layaway deposit against the open shift. Without this the money is in
/// the drawer but the close-out never sees it, so the cut reports a surplus.
fn post_layaway_payment(
    db: &rusqlite::Connection,
    layaway_id: i64,
    amount: f64,
    method: &str,
    user_id: i64,
) -> Result<(), String> {
    let register_id = open_register_id(db);

    // Un abono en efectivo entra al cajón: sin turno abierto no hay dónde
    // registrarlo y el siguiente corte aparecería con un sobrante inexplicable.
    // Mismo criterio que para las devoluciones en efectivo.
    if method == "cash" && register_id.is_none() {
        return Err("Abre la caja antes de recibir un abono en efectivo.".to_string());
    }

    db.execute(
        "INSERT INTO layaway_payments (layaway_id, amount, payment_method, user_id, cash_register_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![layaway_id, amount, method, user_id, register_id],
    )
    .map_err(|e| e.to_string())?;

    if let Some(cr_id) = register_id {
        db.execute(
            &format!(
                "UPDATE cash_registers SET {} = {} + ?1 WHERE id = ?2",
                layaway_register_field(method),
                layaway_register_field(method)
            ),
            params![amount, cr_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn create_layaway(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateLayawayDto) -> Result<Layaway, String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();
    registrar_apartado(&db, user_id, data)
}

/// Núcleo de la creación del apartado, con la conexión explícita.
pub fn registrar_apartado(
    db: &rusqlite::Connection,
    user_id: i64,
    data: CreateLayawayDto,
) -> Result<Layaway, String> {

    if data.items.is_empty() {
        return Err("El apartado no tiene productos".to_string());
    }
    if data.initial_payment < 0.0 {
        return Err("El anticipo no puede ser negativo".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<Layaway, String> {
        let folio = crate::folios::siguiente(db, crate::folios::Serie::Apartados)?;

        struct Line { product_id: i64, name: String, sku: String, quantity: i32, unit_price: f64, unit_cost: f64, subtotal: Cents, variant_id: Option<i64>, variant_label: Option<String> }
        let mut lines: Vec<Line> = Vec::with_capacity(data.items.len());
        let mut total = Cents::ZERO;

        // Lo que los renglones anteriores de este mismo apartado ya reservaron:
        // sin esto, dos tallas del mismo vestido se miden las dos contra la
        // existencia completa.
        let mut apartado_producto: HashMap<i64, i32> = HashMap::new();
        let mut apartado_variante: HashMap<i64, i32> = HashMap::new();

        for item in &data.items {
            if item.quantity <= 0 {
                return Err("Cantidad inválida en un producto".to_string());
            }
            let (name, sku, product_stock, price, cost): (String, String, i32, f64, f64) = db.query_row(
                "SELECT name, sku, stock, sale_price, purchase_price FROM products WHERE id = ?1",
                params![item.product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).map_err(|e| e.to_string())?;

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
                return Err(format!("Stock insuficiente para '{}{}'. Disponible: {}", name, label, available));
            }

            *apartado_producto.entry(item.product_id).or_insert(0) += item.quantity;
            if let Some(vid) = variant_id {
                *apartado_variante.entry(vid).or_insert(0) += item.quantity;
            }

            let subtotal = Cents::from_pesos(price).times(item.quantity as i64);
            total = total + subtotal;
            lines.push(Line { product_id: item.product_id, name, sku, quantity: item.quantity, unit_price: price, unit_cost: cost, subtotal, variant_id, variant_label });
        }
        if Cents::from_pesos(data.initial_payment) > total {
            return Err("El anticipo no puede superar el total".to_string());
        }

        db.execute(
            "INSERT INTO layaways (folio, customer_id, user_id, total, paid, notes, due_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![folio, data.customer_id, user_id, total.to_pesos(), data.initial_payment, data.notes, data.due_date],
        ).map_err(|e| e.to_string())?;
        let layaway_id = db.last_insert_rowid();

        for line in &lines {
            db.execute(
                "INSERT INTO layaway_items (layaway_id, product_id, product_name, product_sku, quantity, unit_price, unit_cost, subtotal, variant_id, variant_label)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![layaway_id, line.product_id, line.name, line.sku, line.quantity, line.unit_price, line.unit_cost, line.subtotal.to_pesos(), line.variant_id, line.variant_label],
            ).map_err(|e| e.to_string())?;

            // Reserve stock (moved out of available inventory).
            //
            // Se lee aquí y se descuenta de forma relativa: con una foto tomada
            // al armar los renglones, dos tallas del mismo vestido partían las
            // dos del mismo número y la segunda deshacía la reserva de la
            // primera.
            let prev_stock: i32 = db
                .query_row("SELECT stock FROM products WHERE id = ?1", params![line.product_id], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let new_stock = prev_stock - line.quantity;
            db.execute(
                "UPDATE products SET stock = stock - ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![line.quantity, line.product_id],
            ).map_err(|e| e.to_string())?;
            if let Some(vid) = line.variant_id {
                db.execute(
                    "UPDATE product_variants SET stock = stock - ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![line.quantity, vid],
                ).map_err(|e| e.to_string())?;
            }
            let reason = line.variant_label.as_ref().map(|l| format!("Apartado reservado ({})", l)).unwrap_or_else(|| "Apartado reservado".to_string());
            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'adjustment', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![line.product_id, -line.quantity, prev_stock, new_stock, layaway_id, reason, user_id],
            ).map_err(|e| e.to_string())?;
        }

        if data.initial_payment > 0.0 {
            post_layaway_payment(db, layaway_id, data.initial_payment, &data.payment_method, user_id)?;
        }

        db.execute(
            "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'layaways', ?1, ?2)",
            params![format!("Apartado {} creado por {}", folio, total), user_id],
        ).ok();

        get_layaway_internal(db, layaway_id)
    })();

    match result {
        Ok(l) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(l)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn get_layaways(state: State<DbState>, sessions: State<SessionState>, token: String, status: Option<String>) -> Result<Vec<Layaway>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let (sql, has_filter) = match status {
        Some(_) => (format!("{} WHERE l.status = ?1 ORDER BY l.created_at DESC", SEL), true),
        None => (format!("{} ORDER BY l.created_at DESC", SEL), false),
    };

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let mapped = if has_filter {
        stmt.query_map(params![status.unwrap()], row_to_layaway)
    } else {
        stmt.query_map([], row_to_layaway)
    };
    let out = mapped
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[tauri::command]
pub fn get_layaway_detail(state: State<DbState>, sessions: State<SessionState>, token: String, layaway_id: i64) -> Result<Layaway, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    get_layaway_internal(&db, layaway_id)
}

#[tauri::command]
pub fn add_layaway_payment(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    layaway_id: i64,
    amount: f64,
    payment_method: String,
) -> Result<Layaway, String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();
    abonar_apartado(&db, user_id, layaway_id, amount, &payment_method)
}

/// Registra un abono validando el estado y el saldo.
///
/// Las reglas viven aquí y no en el comando para que no dependan de quién
/// llame: un abono que supere el saldo dejaría el apartado pagado de más y sin
/// forma de devolver la diferencia.
pub fn abonar_apartado(
    db: &rusqlite::Connection,
    user_id: i64,
    layaway_id: i64,
    amount: f64,
    payment_method: &str,
) -> Result<Layaway, String> {
    if amount <= 0.0 {
        return Err("El abono debe ser mayor a cero".to_string());
    }

    let (status, total, paid): (String, f64, f64) = db.query_row(
        "SELECT status, total, paid FROM layaways WHERE id = ?1",
        params![layaway_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| "El apartado no existe".to_string())?;

    if status != "active" {
        return Err("El apartado no está activo".to_string());
    }

    let saldo = Cents::from_pesos(total) - Cents::from_pesos(paid);
    if Cents::from_pesos(amount) > saldo {
        return Err(format!(
            "El abono supera el saldo pendiente de {}",
            saldo
        ));
    }

    // El abono toca tres tablas: el pago, el corte y el saldo del apartado. Sin
    // transacción, un fallo a la mitad dejaba el dinero contado en la caja y el
    // saldo del cliente sin bajar.
    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        post_layaway_payment(db, layaway_id, amount, payment_method, user_id)?;
        db.execute(
            "UPDATE layaways SET paid = paid + ?1 WHERE id = ?2",
            params![amount, layaway_id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;

    get_layaway_internal(db, layaway_id)
}

#[tauri::command]
pub fn complete_layaway(state: State<DbState>, sessions: State<SessionState>, token: String, layaway_id: i64) -> Result<Layaway, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    entregar_apartado(&db, layaway_id)
}

/// Núcleo de la entrega, con la conexión explícita.
///
/// Entregar es cuando la prenda deja la tienda, así que es cuando se registra
/// la venta. El inventario ya se descontó al apartar y el dinero ya entró al
/// corte abono por abono, así que aquí no se toca ninguno de los dos: solo se
/// deja la venta para que el reporte y las utilidades cuenten lo que de verdad
/// se vendió.
pub fn entregar_apartado(db: &rusqlite::Connection, layaway_id: i64) -> Result<Layaway, String> {
    let (status, total, paid, user_id): (String, f64, f64, i64) = db.query_row(
        "SELECT status, total, paid, user_id FROM layaways WHERE id = ?1",
        params![layaway_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|e| e.to_string())?;

    if status != "active" {
        return Err("El apartado no está activo".to_string());
    }
    if Cents::from_pesos(paid) < Cents::from_pesos(total) {
        return Err("El apartado aún tiene saldo pendiente".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        db.execute(
            "UPDATE layaways SET status = 'completed', completed_at = datetime('now','localtime') WHERE id = ?1",
            params![layaway_id],
        ).map_err(|e| e.to_string())?;
        registrar_venta_de_entrega(db, layaway_id, user_id, total)
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;

    get_layaway_internal(db, layaway_id)
}

/// Deja la venta que corresponde a un apartado entregado.
///
/// Ni inventario ni caja: los dos ya se movieron en su momento. Lo único que
/// falta es la venta, con sus partidas y su costo, para que el reporte diario y
/// la utilidad incluyan lo que salió por apartado.
fn registrar_venta_de_entrega(
    db: &rusqlite::Connection,
    layaway_id: i64,
    user_id: i64,
    total: f64,
) -> Result<(), String> {
    let folio_apartado: String = db
        .query_row("SELECT folio FROM layaways WHERE id = ?1", params![layaway_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let customer_id: Option<i64> = db
        .query_row("SELECT customer_id FROM layaways WHERE id = ?1", params![layaway_id], |r| r.get(0))
        .unwrap_or(None);

    // Cómo pagó: si usó más de una forma a lo largo de los abonos, es mixto.
    let metodos: Vec<String> = db
        .prepare("SELECT DISTINCT payment_method FROM layaway_payments WHERE layaway_id = ?1")
        .and_then(|mut s| s.query_map(params![layaway_id], |r| r.get(0))?.collect())
        .map_err(|e| e.to_string())?;
    let payment_method = match metodos.len() {
        0 => "cash".to_string(),
        1 => metodos[0].clone(),
        _ => "mixed".to_string(),
    };

    let folio = crate::folios::siguiente(db, crate::folios::Serie::Ventas)?;

    db.execute(
        "INSERT INTO sales (folio, user_id, cash_register_id, subtotal, discount_total, tax, total,
                            payment_method, amount_paid, change_amount, notes, customer_id,
                            layaway_id, terminal_id)
         VALUES (?1, ?2, NULL, ?3, 0, 0, ?3, ?4, ?3, 0, ?5, ?6, ?7, ?8)",
        params![
            folio,
            user_id,
            total,
            payment_method,
            format!("Entrega del apartado {}", folio_apartado),
            customer_id,
            layaway_id,
            crate::folios::terminal_id(db)
        ],
    ).map_err(|e| e.to_string())?;
    let sale_id = db.last_insert_rowid();

    // El desglose por forma de pago sale de los abonos, tal como se recibieron.
    let abonos: Vec<(String, f64)> = db
        .prepare(
            "SELECT payment_method, SUM(amount) FROM layaway_payments
             WHERE layaway_id = ?1 GROUP BY payment_method",
        )
        .and_then(|mut s| {
            s.query_map(params![layaway_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
        })
        .map_err(|e| e.to_string())?;
    for (metodo, monto) in abonos {
        db.execute(
            "INSERT INTO sale_payments (sale_id, method, amount) VALUES (?1, ?2, ?3)",
            params![sale_id, metodo, monto],
        ).map_err(|e| e.to_string())?;
    }

    // Las partidas se copian con su costo: sin él la utilidad saldría mal.
    db.execute(
        "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku, quantity,
                                 unit_price, discount, subtotal, unit_cost, variant_id, variant_label)
         SELECT ?1, product_id, product_name, product_sku, quantity,
                unit_price, 0, subtotal, unit_cost, variant_id, variant_label
         FROM layaway_items WHERE layaway_id = ?2",
        params![sale_id, layaway_id],
    ).map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'apartados', ?1, ?2)",
        params![
            format!("Apartado {} entregado y registrado como venta {}", folio_apartado, folio),
            user_id
        ],
    ).ok();

    Ok(())
}

#[tauri::command]
pub fn cancel_layaway(state: State<DbState>, sessions: State<SessionState>, token: String, layaway_id: i64) -> Result<String, String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.conn();
    cancelar_apartado(&db, user_id, layaway_id)
}

/// Núcleo de la cancelación, con la conexión explícita.
pub fn cancelar_apartado(db: &rusqlite::Connection, user_id: i64, layaway_id: i64) -> Result<String, String> {
    // Lo abonado no se mueve solo: si se le regresa a la clienta, ese efectivo
    // sale del cajón y el corte tiene que enterarse. El sistema no puede saber
    // qué se acordó, así que lo dice en voz alta en vez de callarlo.
    let abonado: f64 = db
        .query_row("SELECT paid FROM layaways WHERE id = ?1", params![layaway_id], |r| r.get(0))
        .unwrap_or(0.0);

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        let status: String = db.query_row(
            "SELECT status FROM layaways WHERE id = ?1",
            params![layaway_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if status != "active" {
            return Err("Solo se pueden cancelar apartados activos".to_string());
        }

        // Return reserved stock to inventory.
        let mut stmt = db.prepare(
            "SELECT product_id, quantity, variant_id FROM layaway_items WHERE layaway_id = ?1"
        ).map_err(|e| e.to_string())?;
        let items: Vec<(i64, i32, Option<i64>)> = stmt
            .query_map(params![layaway_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

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
                 VALUES (?1, 'cancellation', ?2, ?3, ?4, ?5, 'Apartado cancelado', ?6)",
                params![product_id, quantity, current_stock, new_stock, layaway_id, user_id],
            ).map_err(|e| e.to_string())?;
        }

        db.execute(
            "UPDATE layaways SET status = 'cancelled' WHERE id = ?1",
            params![layaway_id],
        ).map_err(|e| e.to_string())?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            if abonado > 0.0 {
                db.execute(
                    "INSERT INTO app_logs (level, module, message, user_id)
                     VALUES ('warn', 'apartados', ?1, ?2)",
                    params![
                        format!("Apartado {} cancelado con {:.2} abonados", layaway_id, abonado),
                        user_id
                    ],
                ).ok();
                Ok(format!(
                    "Apartado cancelado y mercancía devuelta al inventario. Tenía {:.2} abonados: si se los regresas en efectivo, anótalo como gasto para que el corte cuadre.",
                    abonado
                ))
            } else {
                Ok("Apartado cancelado y mercancía devuelta al inventario.".to_string())
            }
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

fn get_layaway_internal(db: &rusqlite::Connection, layaway_id: i64) -> Result<Layaway, String> {
    let mut layaway = db.query_row(
        &format!("{} WHERE l.id = ?1", SEL),
        params![layaway_id],
        row_to_layaway,
    ).map_err(|e| e.to_string())?;

    let mut istmt = db.prepare(
        "SELECT id, layaway_id, product_id, product_name, product_sku, quantity, unit_price, subtotal, variant_label FROM layaway_items WHERE layaway_id = ?1"
    ).map_err(|e| e.to_string())?;
    let items = istmt.query_map(params![layaway_id], |row| {
        Ok(LayawayItem {
            id: row.get(0)?,
            layaway_id: row.get(1)?,
            product_id: row.get(2)?,
            product_name: row.get(3)?,
            product_sku: row.get(4)?,
            quantity: row.get(5)?,
            unit_price: row.get(6)?,
            subtotal: row.get(7)?,
            variant_label: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    let mut pstmt = db.prepare(
        "SELECT p.id, p.layaway_id, p.amount, p.payment_method, p.user_id, u.full_name, p.created_at
         FROM layaway_payments p LEFT JOIN users u ON p.user_id = u.id
         WHERE p.layaway_id = ?1 ORDER BY p.created_at ASC"
    ).map_err(|e| e.to_string())?;
    let payments = pstmt.query_map(params![layaway_id], |row| {
        Ok(LayawayPayment {
            id: row.get(0)?,
            layaway_id: row.get(1)?,
            amount: row.get(2)?,
            payment_method: row.get(3)?,
            user_id: row.get(4)?,
            user_name: row.get(5)?,
            created_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    layaway.items = Some(items);
    layaway.payments = Some(payments);
    Ok(layaway)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        db.execute("INSERT INTO layaways (id, folio, user_id, total, paid) VALUES (1, 'A-1', 1, 1000, 0)", []).unwrap();
        db
    }

    #[test]
    fn entregar_un_apartado_lo_deja_registrado_como_venta() {
        // El reporte de ventas no veía los apartados: una tienda que aparta la
        // mitad de lo que vende leía la mitad de lo que vendió.
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "VES", 500.0, 10);

        let ap = registrar_apartado(&db, 1, apartado(p, 2, 400.0, "cash")).unwrap();
        abonar_apartado(&db, 1, ap.id, 600.0, "card").unwrap();
        entregar_apartado(&db, ap.id).unwrap();

        let (total, metodo, partidas): (f64, String, i64) = db.query_row(
            "SELECT s.total, s.payment_method,
                    (SELECT COUNT(*) FROM sale_items WHERE sale_id = s.id)
             FROM sales s WHERE s.layaway_id = ?1",
            params![ap.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();

        assert_eq!(total, 1000.0);
        assert_eq!(metodo, "mixed", "pagó en efectivo y con tarjeta");
        assert_eq!(partidas, 1);

        let costo: f64 = db.query_row(
            "SELECT unit_cost FROM sale_items WHERE sale_id = (SELECT id FROM sales WHERE layaway_id = ?1)",
            params![ap.id], |r| r.get(0)).unwrap();
        assert!(costo > 0.0, "sin costo la utilidad saldría mal");
    }

    #[test]
    fn la_entrega_no_vuelve_a_tocar_el_inventario_ni_la_caja() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "BLU", 300.0, 10);

        let ap = registrar_apartado(&db, 1, apartado(p, 3, 900.0, "cash")).unwrap();
        let stock_apartado: i32 = db.query_row(
            "SELECT stock FROM products WHERE id = ?1", params![p], |r| r.get(0)).unwrap();
        let efectivo_antes = caja_efectivo(&db);

        entregar_apartado(&db, ap.id).unwrap();

        let stock_despues: i32 = db.query_row(
            "SELECT stock FROM products WHERE id = ?1", params![p], |r| r.get(0)).unwrap();
        assert_eq!(stock_despues, stock_apartado, "el inventario se descontó al apartar");
        assert_eq!(caja_efectivo(&db), efectivo_antes, "el dinero entró abono por abono");

        let ventas_en_caja: f64 = db.query_row(
            "SELECT total_sales FROM cash_registers LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(ventas_en_caja, 0.0, "el corte no cuenta la entrega dos veces");
    }

    #[test]
    fn la_venta_de_una_entrega_no_se_cancela_desde_ventas() {
        // Cancelarla devolvería al inventario mercancía que esa venta nunca
        // descontó. Lo que se cancela es el apartado.
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "PAN", 400.0, 10);
        let ap = registrar_apartado(&db, 1, apartado(p, 1, 400.0, "cash")).unwrap();
        entregar_apartado(&db, ap.id).unwrap();

        let venta: i64 = db.query_row(
            "SELECT id FROM sales WHERE layaway_id = ?1", params![ap.id], |r| r.get(0)).unwrap();

        assert!(crate::commands::sales::cancelar_venta(&db, 1, venta).is_err());
    }

    #[test]
    fn muchos_abonos_parciales_cierran_exactamente() {
        // Sumar en coma flotante dejaba un saldo de un centavo imposible de
        // pagar: el apartado quedaba pagado y sin poderse entregar.
        let db = tienda();
        abrir_caja(&db);
        let producto = producto(&db, "TER", 33.33, 30);

        let ap = registrar_apartado(&db, 1, apartado(producto, 3, 0.0, "cash")).unwrap();
        let total = ap.total;

        // Diez abonos de una décima parte, cada uno redondeado a dos decimales.
        let parte = (total / 10.0 * 100.0).round() / 100.0;
        let mut pagado = 0.0;
        for _ in 0..9 {
            abonar_apartado(&db, 1, ap.id, parte, "cash").unwrap();
            pagado += parte;
        }
        let resto = ((total - pagado) * 100.0).round() / 100.0;
        abonar_apartado(&db, 1, ap.id, resto, "cash").unwrap();

        entregar_apartado(&db, ap.id).expect("pagado completo debe poder entregarse");
    }

    #[test]
    fn apartar_dos_tallas_del_mismo_modelo_reserva_las_dos() {
        // Igual que en una venta: dos renglones del mismo producto partían los
        // dos de la misma foto del inventario y el segundo deshacía al primero.
        let db = tienda();
        abrir_caja(&db);
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, has_variants)
             VALUES (7, 'VES', 'Vestido', 200.0, 499.0, 10, 1)", []).unwrap();
        db.execute(
            "INSERT INTO product_variants (id, product_id, size, stock) VALUES (1, 7, 'M', 5), (2, 7, 'G', 5)",
            []).unwrap();

        registrar_apartado(&db, 1, CreateLayawayDto {
            customer_id: None,
            items: vec![
                CreateLayawayItemDto { product_id: 7, quantity: 2, unit_price: 0.0, variant_id: Some(1) },
                CreateLayawayItemDto { product_id: 7, quantity: 3, unit_price: 0.0, variant_id: Some(2) },
            ],
            initial_payment: 0.0,
            payment_method: "cash".into(),
            notes: None,
            due_date: None,
        }).unwrap();

        let total: i32 = db.query_row("SELECT stock FROM products WHERE id = 7", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 5, "el total del producto es la suma de sus tallas");
    }

    #[test]
    fn no_se_aparta_mas_de_lo_que_hay_entre_varios_renglones() {
        let db = tienda();
        abrir_caja(&db);
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (8, 'BLU', 'Blusa', 100.0, 299.0, 5)", []).unwrap();

        let error = registrar_apartado(&db, 1, CreateLayawayDto {
            customer_id: None,
            items: vec![
                CreateLayawayItemDto { product_id: 8, quantity: 3, unit_price: 0.0, variant_id: None },
                CreateLayawayItemDto { product_id: 8, quantity: 3, unit_price: 0.0, variant_id: None },
            ],
            initial_payment: 0.0,
            payment_method: "cash".into(),
            notes: None,
            due_date: None,
        });

        assert!(error.is_err(), "seis piezas de cinco no deberían apartarse");
        let quedan: i32 = db.query_row("SELECT stock FROM products WHERE id = 8", [], |r| r.get(0)).unwrap();
        assert_eq!(quedan, 5, "un apartado rechazado no toca el inventario");
    }

    fn abrir_caja(db: &rusqlite::Connection) {
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 0)", []).unwrap();
    }

    fn caja_efectivo(db: &rusqlite::Connection) -> f64 {
        db.query_row("SELECT total_layaway_cash FROM cash_registers LIMIT 1", [], |r| r.get(0)).unwrap()
    }

    use crate::models::layaway::{CreateLayawayDto, CreateLayawayItemDto};

    fn producto(db: &rusqlite::Connection, sku: &str, precio: f64, stock: i32) -> i64 {
        db.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
             VALUES (?1, ?1, 10.0, ?2, ?3)",
            params![sku, precio, stock],
        ).unwrap();
        db.last_insert_rowid()
    }

    fn stock(db: &rusqlite::Connection, id: i64) -> i32 {
        db.query_row("SELECT stock FROM products WHERE id = ?1", params![id], |r| r.get(0)).unwrap()
    }

    fn apartado(product_id: i64, cantidad: i32, anticipo: f64, metodo: &str) -> CreateLayawayDto {
        CreateLayawayDto {
            customer_id: None,
            notes: None,
            due_date: None,
            initial_payment: anticipo,
            payment_method: metodo.to_string(),
            items: vec![CreateLayawayItemDto {
                product_id, quantity: cantidad, unit_price: 0.0, variant_id: None,
            }],
        }
    }

    #[test]
    fn crear_un_apartado_reserva_el_inventario() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);

        let l = registrar_apartado(&db, 1, apartado(p, 3, 100.0, "cash")).unwrap();

        assert_eq!(l.total, 300.0);
        assert_eq!(l.paid, 100.0);
        assert_eq!(stock(&db, p), 7, "el apartado saca la mercancía del inventario");
    }

    #[test]
    fn el_anticipo_no_puede_superar_el_total() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);

        assert!(registrar_apartado(&db, 1, apartado(p, 1, 500.0, "cash")).is_err());
        assert_eq!(stock(&db, p), 10, "nada debe reservarse si el apartado no se creó");
    }

    #[test]
    fn no_se_aparta_mas_de_lo_que_hay_en_existencia() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 2);

        assert!(registrar_apartado(&db, 1, apartado(p, 5, 0.0, "cash")).is_err());
        assert_eq!(stock(&db, p), 2);
    }

    #[test]
    fn el_anticipo_en_efectivo_entra_a_la_caja_al_crear_el_apartado() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);

        registrar_apartado(&db, 1, apartado(p, 2, 50.0, "cash")).unwrap();

        assert_eq!(caja_efectivo(&db), 50.0);
    }

    #[test]
    fn no_se_puede_abonar_mas_que_el_saldo() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 1, 60.0, "cash")).unwrap();

        assert!(abonar_apartado(&db, 1, l.id, 100.0, "cash").is_err(),
                "el saldo es 40, no puede abonar 100");
        assert!(abonar_apartado(&db, 1, l.id, 40.0, "cash").is_ok(),
                "abonar exactamente el saldo sí debe poder");
    }

    #[test]
    fn no_se_entrega_un_apartado_con_saldo_pendiente() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 1, 50.0, "cash")).unwrap();

        assert!(entregar_apartado(&db, l.id).is_err());
    }

    #[test]
    fn liquidar_permite_entregar_y_no_devuelve_el_inventario() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 2, 100.0, "cash")).unwrap();

        db.execute("UPDATE layaways SET paid = total WHERE id = ?1", params![l.id]).unwrap();
        let entregado = entregar_apartado(&db, l.id).unwrap();

        assert_eq!(entregado.status, "completed");
        assert_eq!(stock(&db, p), 8, "la mercancía se la llevó el cliente");
    }

    #[test]
    fn cancelar_un_apartado_regresa_el_inventario() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 3, 100.0, "cash")).unwrap();
        assert_eq!(stock(&db, p), 7);

        cancelar_apartado(&db, 1, l.id).unwrap();

        assert_eq!(stock(&db, p), 10, "la mercancía vuelve a estar disponible");
        let estado: String = db.query_row(
            "SELECT status FROM layaways WHERE id = ?1", params![l.id], |r| r.get(0)).unwrap();
        assert_eq!(estado, "cancelled");
    }

    #[test]
    fn no_se_cancela_dos_veces_un_apartado() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 1, 0.0, "cash")).unwrap();

        cancelar_apartado(&db, 1, l.id).unwrap();
        assert!(cancelar_apartado(&db, 1, l.id).is_err(),
                "cancelar de nuevo duplicaría el inventario devuelto");
        assert_eq!(stock(&db, p), 10);
    }

    #[test]
    fn no_se_entrega_un_apartado_ya_cancelado() {
        let db = tienda();
        abrir_caja(&db);
        let p = producto(&db, "CAM", 100.0, 10);
        let l = registrar_apartado(&db, 1, apartado(p, 1, 0.0, "cash")).unwrap();
        cancelar_apartado(&db, 1, l.id).unwrap();

        assert!(entregar_apartado(&db, l.id).is_err());
    }

    #[test]
    fn un_abono_en_efectivo_entra_a_la_caja_del_turno() {
        let db = tienda();
        abrir_caja(&db);

        post_layaway_payment(&db, 1, 500.0, "cash", 1).unwrap();

        assert_eq!(caja_efectivo(&db), 500.0);
    }

    #[test]
    fn un_abono_con_tarjeta_no_toca_el_efectivo() {
        let db = tienda();
        abrir_caja(&db);

        post_layaway_payment(&db, 1, 500.0, "card", 1).unwrap();

        assert_eq!(caja_efectivo(&db), 0.0);
        let tarjeta: f64 = db.query_row(
            "SELECT total_layaway_card FROM cash_registers LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(tarjeta, 500.0);
    }

    #[test]
    fn sin_caja_abierta_no_se_recibe_efectivo() {
        let db = tienda();

        let err = post_layaway_payment(&db, 1, 500.0, "cash", 1).unwrap_err();
        assert!(err.contains("Abre la caja"), "mensaje poco claro: {}", err);

        let abonos: i64 = db.query_row("SELECT COUNT(*) FROM layaway_payments", [], |r| r.get(0)).unwrap();
        assert_eq!(abonos, 0, "no debe quedar registrado un abono que no se pudo ubicar");
    }

    #[test]
    fn sin_caja_abierta_si_se_admite_una_transferencia() {
        // No pasa por el cajón, así que no descuadra nada.
        let db = tienda();
        assert!(post_layaway_payment(&db, 1, 500.0, "transfer", 1).is_ok());
    }

    #[test]
    fn el_abono_queda_ligado_al_turno_en_que_ocurrio() {
        let db = tienda();
        abrir_caja(&db);
        post_layaway_payment(&db, 1, 500.0, "cash", 1).unwrap();

        let cr: Option<i64> = db.query_row(
            "SELECT cash_register_id FROM layaway_payments LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(cr, Some(1));
    }

    #[test]
    fn cada_metodo_suma_en_su_propia_columna() {
        assert_eq!(layaway_register_field("cash"), "total_layaway_cash");
        assert_eq!(layaway_register_field("card"), "total_layaway_card");
        assert_eq!(layaway_register_field("transfer"), "total_layaway_transfer");
        assert_eq!(layaway_register_field("otro"), "total_layaway_cash");
    }
}
