import { useCallback, useEffect, useState } from 'react';
import * as api from '../api';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';
import type { Category } from '../types';

/**
 * Administración de categorías.
 *
 * Existe porque el catálogo de una tienda cambia con las temporadas y nadie
 * debería depender de que alguien toque la base para agregar "Trajes de baño".
 */
export default function CategorySettings() {
    const { showToast } = useToast();
    const { confirm } = useConfirm();
    const [categorias, setCategorias] = useState<Category[]>([]);
    const [nueva, setNueva] = useState('');
    const [editando, setEditando] = useState<number | null>(null);
    const [nombreEditado, setNombreEditado] = useState('');
    const [ocupado, setOcupado] = useState(false);

    const cargar = useCallback(async () => {
        try { setCategorias(await api.getCategories()); }
        catch (err) { showToast(String(err), 'error'); }
    }, [showToast]);

    useEffect(() => { cargar(); }, [cargar]);

    const agregar = async () => {
        const nombre = nueva.trim();
        if (!nombre) return;
        setOcupado(true);
        try {
            await api.createCategory({ name: nombre, description: null });
            setNueva('');
            await cargar();
            showToast(`"${nombre}" agregada`);
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    const guardarNombre = async (c: Category) => {
        const nombre = nombreEditado.trim();
        if (!nombre || nombre === c.name) { setEditando(null); return; }
        setOcupado(true);
        try {
            await api.updateCategory({ id: c.id, name: nombre, description: c.description, is_active: c.is_active });
            setEditando(null);
            await cargar();
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    const quitar = async (c: Category) => {
        const tiene = c.product_count ?? 0;
        if (tiene > 0) {
            showToast(
                `"${c.name}" tiene ${tiene} producto${tiene === 1 ? '' : 's'}. Cámbialos de categoría antes de quitarla.`,
                'error',
            );
            return;
        }
        const ok = await confirm({
            title: `Quitar "${c.name}"`,
            message: `Se quitará "${c.name}" de la lista. ¿Continuar?`,
            variant: 'danger',
            confirmLabel: 'Quitar',
        });
        if (!ok) return;
        setOcupado(true);
        try {
            await api.deleteCategory(c.id);
            await cargar();
            showToast(`"${c.name}" quitada`);
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    return (
        <div className="card" style={{ padding: '22px 24px' }}>
            <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>
                Categorías
            </p>
            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 16, lineHeight: 1.5 }}>
                Son los grupos en los que se acomoda la ropa. Agrega los que uses en la
                tienda y quita los que no.
            </p>

            <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
                <input
                    value={nueva}
                    onChange={e => setNueva(e.target.value)}
                    onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); agregar(); } }}
                    placeholder="Trajes de baño"
                    className="input"
                    style={{ flex: 1 }}
                />
                <button onClick={agregar} disabled={ocupado || !nueva.trim()} className="btn btn-primary btn-sm">
                    Agregar
                </button>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 2, maxHeight: 300, overflowY: 'auto' }}>
                {categorias.map(c => (
                    <div key={c.id} style={{
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '9px 12px', borderRadius: 10, background: 'rgba(255,255,255,0.03)',
                    }}>
                        {editando === c.id ? (
                            <input
                                value={nombreEditado}
                                onChange={e => setNombreEditado(e.target.value)}
                                onKeyDown={e => { if (e.key === 'Enter') guardarNombre(c); if (e.key === 'Escape') setEditando(null); }}
                                onBlur={() => guardarNombre(c)}
                                className="input"
                                style={{ flex: 1, padding: '6px 10px' }}
                                autoFocus
                            />
                        ) : (
                            <>
                                <span
                                    onClick={() => { setEditando(c.id); setNombreEditado(c.name); }}
                                    style={{ flex: 1, fontSize: 13, color: 'var(--t1)', cursor: 'text' }}
                                >
                                    {c.name}
                                </span>
                                <span style={{ fontSize: 11, color: 'var(--t3)' }}>
                                    {(c.product_count ?? 0) === 0
                                        ? 'sin productos'
                                        : `${c.product_count} producto${c.product_count === 1 ? '' : 's'}`}
                                </span>
                                <button
                                    onClick={() => quitar(c)}
                                    disabled={ocupado}
                                    style={{ background: 'none', border: 'none', color: 'var(--t3)', cursor: 'pointer', padding: 4, fontSize: 15 }}
                                    aria-label={`Quitar ${c.name}`}
                                >
                                    &times;
                                </button>
                            </>
                        )}
                    </div>
                ))}
                {categorias.length === 0 && (
                    <p style={{ fontSize: 12, color: 'var(--t3)', textAlign: 'center', padding: '20px 0' }}>
                        Todavía no hay categorías
                    </p>
                )}
            </div>
        </div>
    );
}
