use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::customer::{Customer, CreateCustomerDto, UpdateCustomerDto};
use crate::commands::fiscal::validar_datos_fiscales;
use crate::session::{require_admin, require_auth, SessionState};

/// Lo que un cliente lleva comprado, **ya descontado lo que devolvió**.
///
/// Sumar `total` a secas contaba como compra el dinero que se le regresó: quien
/// compró cinco mil y lo devolvió todo seguía apareciendo con cinco mil. Es el
/// mismo criterio que usan los reportes.
const SEL: &str = "SELECT c.id, c.name, c.phone, c.email, c.notes,
    c.rfc, c.razon_social, c.regimen_fiscal, c.cp_fiscal, c.uso_cfdi,
    c.is_active, c.created_at, c.updated_at,
    COALESCE((SELECT SUM(s.total - COALESCE(
                 (SELECT SUM(r.total_refund) FROM returns r WHERE r.sale_id = s.id), 0))
              FROM sales s
              WHERE s.customer_id = c.id AND s.status IN ('completed', 'returned')), 0) AS total_purchases,
    COALESCE((SELECT COUNT(*) FROM sales WHERE customer_id = c.id AND status = 'completed'), 0) AS purchase_count
    FROM customers c";

fn row_to_customer(row: &rusqlite::Row) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        phone: row.get(2)?,
        email: row.get(3)?,
        notes: row.get(4)?,
        rfc: row.get(5)?,
        razon_social: row.get(6)?,
        regimen_fiscal: row.get(7)?,
        cp_fiscal: row.get(8)?,
        uso_cfdi: row.get(9)?,
        is_active: row.get::<_, i32>(10)? == 1,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        total_purchases: row.get(13)?,
        purchase_count: row.get(14)?,
    })
}

#[tauri::command]
pub fn get_customers(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Customer>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    let mut stmt = db.prepare(&format!("{} ORDER BY c.name ASC", SEL)).map_err(|e| e.to_string())?;
    let out = stmt
        .query_map([], row_to_customer)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[tauri::command]
pub fn create_customer(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateCustomerDto) -> Result<Customer, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    if data.name.trim().is_empty() {
        return Err("El nombre es requerido".to_string());
    }
    // Un RFC mal capturado se descubre al facturar, semanas después; mejor aquí.
    validar_datos_fiscales(&data.rfc, &data.regimen_fiscal, &data.cp_fiscal, &data.uso_cfdi)?;

    db.execute(
        "INSERT INTO customers (name, phone, email, notes, rfc, razon_social, regimen_fiscal, cp_fiscal, uso_cfdi)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            data.name, data.phone, data.email, data.notes,
            data.rfc, data.razon_social, data.regimen_fiscal, data.cp_fiscal, data.uso_cfdi
        ],
    ).map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    db.query_row(&format!("{} WHERE c.id = ?1", SEL), params![id], row_to_customer)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_customer(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateCustomerDto) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    if data.name.trim().is_empty() {
        return Err("El nombre es requerido".to_string());
    }
    validar_datos_fiscales(&data.rfc, &data.regimen_fiscal, &data.cp_fiscal, &data.uso_cfdi)?;

    db.execute(
        "UPDATE customers SET name=?1, phone=?2, email=?3, notes=?4,
                rfc=?5, razon_social=?6, regimen_fiscal=?7, cp_fiscal=?8, uso_cfdi=?9,
                is_active=?10, updated_at=datetime('now','localtime')
         WHERE id=?11",
        params![
            data.name, data.phone, data.email, data.notes,
            data.rfc, data.razon_social, data.regimen_fiscal, data.cp_fiscal, data.uso_cfdi,
            data.is_active as i32, data.id
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_customer(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    db.execute(
        "UPDATE customers SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::returns::{registrar_devolucion, CreateReturnDto, ReturnItemDto};
    use crate::commands::sales::registrar_venta;
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto};

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 0)", []).unwrap();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'P', 'P', 50.0, 100.0, 100)",
            [],
        ).unwrap();
        db.execute("INSERT INTO customers (id, name) VALUES (1, 'Ana')", []).unwrap();
        db
    }

    fn comprar(db: &rusqlite::Connection, piezas: i32) -> i64 {
        registrar_venta(db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id: 1, quantity: piezas, unit_price: 0.0, discount: 0.0, variant_id: None,
            }],
            payment_method: "cash".to_string(),
            amount_paid: 100_000.0,
            payments: vec![],
            discount_total: 0.0,
            promotion_id: None,
            requiere_factura: false,
            notes: None,
            customer_id: Some(1),
            client_request_id: None,
        }).unwrap().id
    }

    fn devolver(db: &rusqlite::Connection, venta: i64, piezas: i32) {
        let partida: i64 = db
            .query_row("SELECT id FROM sale_items WHERE sale_id = ?1", params![venta], |r| r.get(0))
            .unwrap();
        registrar_devolucion(db, 1, CreateReturnDto {
            sale_id: venta,
            reason: None,
            refund_method: "cash".to_string(),
            items: vec![ReturnItemDto { sale_item_id: partida, quantity: piezas }],
        }).unwrap();
    }

    fn comprado(db: &rusqlite::Connection) -> f64 {
        db.query_row(&format!("{} WHERE c.id = 1", SEL), [], row_to_customer)
            .unwrap()
            .total_purchases
            .unwrap_or(0.0)
    }

    #[test]
    fn lo_comprado_descuenta_lo_devuelto() {
        let db = tienda();
        let venta = comprar(&db, 3);
        assert_eq!(comprado(&db), 300.0);

        devolver(&db, venta, 1);

        assert_eq!(comprado(&db), 200.0, "lo devuelto no es una compra");
    }

    #[test]
    fn devolverlo_todo_deja_al_cliente_en_cero() {
        let db = tienda();
        let venta = comprar(&db, 2);

        devolver(&db, venta, 2);

        assert_eq!(comprado(&db), 0.0);
    }

    #[test]
    fn sin_devoluciones_no_cambia_nada() {
        let db = tienda();
        comprar(&db, 2);
        assert_eq!(comprado(&db), 200.0);
    }
}
