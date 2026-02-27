import { useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import {
    ShoppingBag,
    Eye,
    EyeOff,
    Loader2,
} from 'lucide-react';

export default function LoginPage() {
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [showPassword, setShowPassword] = useState(false);
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const setSession = useSessionStore((s) => s.setSession);

    const handleLogin = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!username || !password) {
            setError('Ingresa usuario y contraseña');
            return;
        }

        setLoading(true);
        setError('');

        try {
            const response = await api.login({ username, password });
            setSession(response.user, response.token);
        } catch (err) {
            setError(String(err));
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="h-full flex items-center justify-center bg-bg-primary">
            <div className="w-full max-w-lg animate-fade-in">
                {/* Logo */}
                <div className="text-center mb-10">
                    <div className="inline-flex items-center justify-center w-24 h-24 rounded-2xl bg-primary/10 border border-primary/20 mb-5">
                        <ShoppingBag className="w-12 h-12 text-primary" />
                    </div>
                    <h1 className="text-3xl font-bold text-text-primary">Things Shop</h1>
                    <p className="text-text-secondary mt-1.5 text-base">Punto de Venta</p>
                </div>

                {/* Login Form */}
                <div className="glass rounded-2xl p-10">
                    <form onSubmit={handleLogin} className="space-y-6">
                        <div>
                            <label className="block text-sm font-medium text-text-secondary mb-2">
                                Usuario
                            </label>
                            <input
                                type="text"
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary placeholder-text-muted focus:border-primary transition-colors"
                                placeholder="Ingresa tu usuario"
                                autoFocus
                            />
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-text-secondary mb-2">
                                Contraseña
                            </label>
                            <div className="relative">
                                <input
                                    type={showPassword ? 'text' : 'password'}
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary placeholder-text-muted focus:border-primary transition-colors pr-14"
                                    placeholder="Ingresa tu contraseña"
                                />
                                <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="absolute right-4 top-1/2 -translate-y-1/2 text-text-muted hover:text-text-secondary transition-colors"
                                >
                                    {showPassword ? <EyeOff size={20} /> : <Eye size={20} />}
                                </button>
                            </div>
                        </div>

                        {error && (
                            <div className="bg-danger/10 border border-danger/20 text-danger rounded-2xl px-5 py-4 text-sm animate-fade-in">
                                {error}
                            </div>
                        )}

                        <button
                            type="submit"
                            disabled={loading}
                            className="w-full py-4 bg-primary hover:bg-primary-hover text-white text-lg font-semibold rounded-2xl transition-all duration-200 flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {loading ? (
                                <Loader2 className="w-5 h-5 animate-spin" />
                            ) : (
                                'Iniciar Sesión'
                            )}
                        </button>
                    </form>

                    <div className="mt-8 pt-5 border-t border-border">
                        <p className="text-xs text-text-muted text-center">
                            Primera vez? Usuario: <span className="text-accent">admin</span> / Contraseña: <span className="text-accent">admin123</span>
                        </p>
                    </div>
                </div>
            </div>
        </div>
    );
}
