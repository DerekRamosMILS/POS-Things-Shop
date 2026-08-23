use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::variant::{ProductVariant, SaveVariantDto};
use crate::session::{require_admin, require_auth, SessionState};

fn row_to_variant(row: &rusqlite::Row) -> rusqlite::Result<ProductVariant> {
    Ok(ProductVariant {
        id: row.get(0)?,
        product_id: row.get(1)?,
        size: row.get(2)?,
        color: row.get(3)?,
        sku: row.get(4)?,
        barcode: row.get(5)?,
        stock: row.get(6)?,
        is_active: row.get::<_, i32>(7)? == 1,
    })
}

const SEL: &str = "SELECT id, product_id, size, color, sku, barcode, stock, is_active FROM product_variants";

#[tauri::command]
pub fn get_variants(state: State<DbState>, sessions: State<SessionState>, token: String, product_id: i64) -> Result<Vec<ProductVariant>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(&format!("{} WHERE product_id = ?1 AND is_active = 1 ORDER BY id ASC", SEL))
        .map_err(|e| e.to_string())?;
    let out = stmt
        .query_map(params![product_id], row_to_variant)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[tauri::command]
pub fn get_variant_by_barcode(state: State<DbState>, sessions: State<SessionState>, token: String, barcode: String) -> Result<Option<ProductVariant>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let result = db.query_row(
        &format!("{} WHERE barcode = ?1 AND is_active = 1", SEL),
        params![barcode],
        row_to_variant,
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Replace the variant set for a product. Existing variants missing from the
/// incoming list are soft-deleted (kept for historical sales). Afterwards the
/// product's aggregate stock and has_variants flag are recomputed.
#[tauri::command]
pub fn save_variants(state: State<DbState>, sessions: State<SessionState>, token: String, product_id: i64, variants: Vec<SaveVariantDto>) -> Result<Vec<ProductVariant>, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let norm = |s: &Option<String>| -> Option<String> {
        s.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    };

    let result = (|| -> Result<(), String> {
        // Soft-delete variants that are no longer present.
        let incoming_ids: Vec<i64> = variants.iter().filter_map(|v| v.id).collect();
        let mut stmt = db.prepare("SELECT id FROM product_variants WHERE product_id = ?1 AND is_active = 1")
            .map_err(|e| e.to_string())?;
        let existing: Vec<i64> = stmt
            .query_map(params![product_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for id in existing {
            if !incoming_ids.contains(&id) {
                db.execute("UPDATE product_variants SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
            }
        }

        let dup_err = |e: rusqlite::Error| {
            let s = e.to_string();
            if s.contains("UNIQUE") { "El SKU o código de barras de una variante ya existe".to_string() } else { s }
        };

        for v in &variants {
            let size = norm(&v.size);
            let color = norm(&v.color);
            let sku = norm(&v.sku);
            let barcode = norm(&v.barcode);
            let stock = v.stock.max(0);
            match v.id {
                Some(id) => {
                    db.execute(
                        "UPDATE product_variants SET size=?1, color=?2, sku=?3, barcode=?4, stock=?5, is_active=1, updated_at=datetime('now','localtime') WHERE id=?6 AND product_id=?7",
                        params![size, color, sku, barcode, stock, id, product_id],
                    ).map_err(dup_err)?;
                }
                None => {
                    db.execute(
                        "INSERT INTO product_variants (product_id, size, color, sku, barcode, stock) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![product_id, size, color, sku, barcode, stock],
                    ).map_err(dup_err)?;
                }
            }
        }

        // Recompute aggregate stock + flag.
        let (count, sum): (i64, i32) = db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(stock), 0) FROM product_variants WHERE product_id = ?1 AND is_active = 1",
            params![product_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|e| e.to_string())?;

        if count > 0 {
            db.execute(
                "UPDATE products SET has_variants = 1, stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![sum, product_id],
            ).map_err(|e| e.to_string())?;
        } else {
            db.execute(
                "UPDATE products SET has_variants = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![product_id],
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            let mut stmt = db
                .prepare(&format!("{} WHERE product_id = ?1 AND is_active = 1 ORDER BY id ASC", SEL))
                .map_err(|e| e.to_string())?;
            let out = stmt
                .query_map(params![product_id], row_to_variant)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}
