import { useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';

export default function LoginPage() {
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [showPass, setShowPass] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const setSession = useSessionStore((s) => s.setSession);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!username || !password) { setError('Ingresa usuario y contraseña'); return; }
        setLoading(true); setError('');
        try {
            const response = await api.login({ username, password });
            setSession(response.user, response.token);
        } catch (err) {
            setError(String(err));
        } finally {
            setLoading(false);
        }
    };

    const inputStyle: React.CSSProperties = {
        width: '100%', padding: '12px 14px', borderRadius: 13, fontSize: 14,
        background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.10)',
        color: 'var(--t1)', fontFamily: 'inherit', outline: 'none',
        transition: 'border-color 0.15s',
    };

    return (
        <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--bg)' }}>
            {/* Blobs */}
            <div className="blobs" />

            <div style={{ width: '100%', maxWidth: 400, padding: '0 24px', position: 'relative', zIndex: 1 }} className="animate-in">
                {/* Brand */}
                <div style={{ textAlign: 'center', marginBottom: 40 }}>
                    <div style={{
                        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                        width: 64, height: 64, borderRadius: 20, marginBottom: 16,
                        background: 'linear-gradient(135deg, #8B78F5, #F0C547)',
                        boxShadow: '0 12px 40px rgba(139,120,245,0.45)',
                    }}>
                        <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M6 2L3 6v14a2 2 0 002 2h14a2 2 0 002-2V6l-3-4z"/>
                            <line x1="3" y1="6" x2="21" y2="6"/>
                            <path d="M16 10a4 4 0 01-8 0"/>
                        </svg>
                    </div>
                    <h1 style={{ fontSize: 24, fontWeight: 900, color: 'var(--t1)', letterSpacing: '-0.03em', marginBottom: 4 }}>ThingsShop</h1>
                    <p style={{ fontSize: 13, color: 'var(--t3)', fontWeight: 500 }}>Sistema de Punto de Venta · Ropa</p>
                </div>

                {/* Card */}
                <div className="glass-modal" style={{ padding: 32 }}>
                    <h2 style={{ fontSize: 18, fontWeight: 800, color: 'var(--t1)', marginBottom: 4 }}>Iniciar sesión</h2>
                    <p style={{ fontSize: 13, color: 'var(--t3)', marginBottom: 28 }}>Bienvenido de vuelta 👋</p>

                    <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                        <div>
                            <label className="form-label">Usuario</label>
                            <input
                                value={username} onChange={e => setUsername(e.target.value)}
                                placeholder="admin" autoFocus style={inputStyle}
                                onFocus={e => (e.target.style.borderColor = 'var(--primary)')}
                                onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.10)')}
                            />
                        </div>
                        <div>
                            <label className="form-label">Contraseña</label>
                            <div style={{ position: 'relative' }}>
                                <input
                                    type={showPass ? 'text' : 'password'}
                                    value={password} onChange={e => setPassword(e.target.value)}
                                    placeholder="••••••••"
                                    style={{ ...inputStyle, paddingRight: 44 }}
                                    onFocus={e => (e.target.style.borderColor = 'var(--primary)')}
                                    onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.10)')}
                                />
                                <button
                                    type="button" onClick={() => setShowPass(!showPass)}
                                    style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', padding: 4, display: 'grid', placeItems: 'center' }}
                                >
                                    {showPass
                                        ? <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/><path d="M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19"/><line x1="1" y1="1" x2="23" y2="23"/></svg>
                                        : <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
                                    }
                                </button>
                            </div>
                        </div>

                        {error && (
                            <div style={{ padding: '10px 14px', borderRadius: 11, fontSize: 13, background: 'rgba(244,82,112,0.10)', border: '1px solid rgba(244,82,112,0.25)', color: 'var(--danger)' }}>
                                {error}
                            </div>
                        )}

                        <button
                            type="submit" disabled={loading}
                            style={{
                                width: '100%', padding: '14px', borderRadius: 14, border: 'none',
                                background: 'linear-gradient(135deg, #8B78F5, #6B56E0)',
                                color: '#fff', fontSize: 15, fontWeight: 700,
                                cursor: loading ? 'not-allowed' : 'pointer',
                                boxShadow: '0 8px 28px rgba(139,120,245,0.4)',
                                fontFamily: 'inherit', marginTop: 4,
                                opacity: loading ? 0.75 : 1,
                                display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                                transition: 'all 0.15s',
                            }}
                        >
                            {loading
                                ? <><div style={{ width: 16, height: 16, border: '2px solid rgba(255,255,255,0.35)', borderTopColor: '#fff', borderRadius: '50%', animation: 'spin 0.7s linear infinite' }} />Verificando...</>
                                : 'Iniciar sesión →'
                            }
                        </button>
                    </form>

                    <div style={{ marginTop: 20, padding: '10px 14px', borderRadius: 11, background: 'rgba(139,120,245,0.07)', border: '1px solid rgba(139,120,245,0.15)', textAlign: 'center' }}>
                        <p style={{ fontSize: 11, color: 'var(--t3)' }}>Demo: usa <strong style={{ color: 'var(--t2)' }}>admin / 1234</strong> ó <strong style={{ color: 'var(--t2)' }}>cajero / 1234</strong></p>
                    </div>
                </div>
            </div>
        </div>
    );
}
