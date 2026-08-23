import { useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import { useToast } from '../contexts/ToastContext';
import * as api from '../api';

const MIN_LENGTH = 8;

/**
 * Blocking screen shown when the backend flags the account with
 * `must_change_password` — a freshly seeded admin or an admin-reset password.
 */
export default function ChangePasswordPage() {
    const { user, setSession, token, logout } = useSessionStore();
    const { showToast } = useToast();
    const [current, setCurrent] = useState('');
    const [next, setNext] = useState('');
    const [confirm, setConfirm] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (next.length < MIN_LENGTH) {
            setError(`La nueva contraseña debe tener al menos ${MIN_LENGTH} caracteres`);
            return;
        }
        if (next !== confirm) { setError('Las contraseñas no coinciden'); return; }

        setLoading(true); setError('');
        try {
            await api.changeOwnPassword({ current_password: current, new_password: next });
            if (user && token) setSession({ ...user, must_change_password: false }, token);
            showToast('Contraseña actualizada', 'success');
        } catch (err) {
            setError(String(err));
        } finally {
            setLoading(false);
        }
    };

    const inputStyle: React.CSSProperties = {
        width: '100%', padding: '12px 14px', borderRadius: 13, fontSize: 14,
        background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.10)',
        color: 'var(--t1)', fontFamily: 'inherit', outline: 'none', marginBottom: 14,
    };

    return (
        <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--bg)' }}>
            <div className="blobs" />
            <div style={{ width: '100%', maxWidth: 420, padding: '0 24px', position: 'relative', zIndex: 1 }} className="animate-in">
                <div className="glass-modal" style={{ padding: 32 }}>
                    <h2 style={{ fontSize: 18, fontWeight: 800, color: 'var(--t1)', marginBottom: 4 }}>
                        Cambia tu contraseña
                    </h2>
                    <p style={{ fontSize: 13, color: 'var(--t3)', marginBottom: 20 }}>
                        Por seguridad no puedes seguir con la contraseña asignada. Elige una
                        de al menos {MIN_LENGTH} caracteres.
                    </p>

                    <form onSubmit={submit}>
                        <label style={{ fontSize: 12, color: 'var(--t3)' }}>Contraseña actual</label>
                        <input type="password" value={current} autoFocus
                            onChange={(e) => setCurrent(e.target.value)} style={inputStyle} />

                        <label style={{ fontSize: 12, color: 'var(--t3)' }}>Nueva contraseña</label>
                        <input type="password" value={next}
                            onChange={(e) => setNext(e.target.value)} style={inputStyle} />

                        <label style={{ fontSize: 12, color: 'var(--t3)' }}>Confirmar nueva contraseña</label>
                        <input type="password" value={confirm}
                            onChange={(e) => setConfirm(e.target.value)} style={inputStyle} />

                        {error && (
                            <div style={{ padding: '10px 14px', borderRadius: 11, marginBottom: 14, background: 'rgba(255,107,129,0.10)', border: '1px solid rgba(255,107,129,0.25)' }}>
                                <p style={{ fontSize: 12, color: 'var(--danger)' }}>{error}</p>
                            </div>
                        )}

                        <button type="submit" disabled={loading} className="btn btn-primary"
                            style={{ width: '100%', padding: 14, marginTop: 4 }}>
                            {loading ? 'Guardando...' : 'Guardar y continuar'}
                        </button>
                    </form>

                    <button onClick={logout}
                        style={{ marginTop: 14, width: '100%', background: 'none', border: 'none', color: 'var(--t3)', fontSize: 12, cursor: 'pointer', fontFamily: 'inherit' }}>
                        Cerrar sesión
                    </button>
                </div>
            </div>
        </div>
    );
}
