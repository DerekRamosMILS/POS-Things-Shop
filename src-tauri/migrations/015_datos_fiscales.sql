-- Things Shop POS - Datos fiscales para facturación (CFDI 4.0)
-- Version: 015
--
-- Esto NO timbra facturas: el timbrado requiere un contrato con un PAC y los
-- certificados de sello digital del negocio. Lo que sí resuelve es capturar y
-- conservar los datos que la factura exige, para poder entregarlos al PAC o al
-- contador sin perseguir al cliente después de la venta.

ALTER TABLE customers ADD COLUMN rfc TEXT;
ALTER TABLE customers ADD COLUMN razon_social TEXT;
ALTER TABLE customers ADD COLUMN regimen_fiscal TEXT;
ALTER TABLE customers ADD COLUMN cp_fiscal TEXT;
ALTER TABLE customers ADD COLUMN uso_cfdi TEXT;

CREATE INDEX IF NOT EXISTS idx_customers_rfc ON customers(rfc);

-- El cliente puede cambiar sus datos después; la factura debe emitirse con los
-- que estaban vigentes al momento de la venta, así que se guarda una copia.
ALTER TABLE sales ADD COLUMN requiere_factura INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sales ADD COLUMN fiscal_rfc TEXT;
ALTER TABLE sales ADD COLUMN fiscal_razon_social TEXT;
ALTER TABLE sales ADD COLUMN fiscal_regimen TEXT;
ALTER TABLE sales ADD COLUMN fiscal_cp TEXT;
ALTER TABLE sales ADD COLUMN fiscal_uso_cfdi TEXT;
-- Folio fiscal (UUID) que devuelve el PAC una vez timbrada.
ALTER TABLE sales ADD COLUMN uuid_fiscal TEXT;
ALTER TABLE sales ADD COLUMN facturada_at TEXT;

CREATE INDEX IF NOT EXISTS idx_sales_factura ON sales(requiere_factura, uuid_fiscal);

INSERT OR IGNORE INTO system_config (key, value, description) VALUES
    ('rfc_emisor', '', 'RFC del negocio que emite las facturas'),
    ('regimen_emisor', '', 'Régimen fiscal del negocio (clave del SAT)'),
    ('cp_emisor', '', 'Código postal del domicilio fiscal del negocio');
