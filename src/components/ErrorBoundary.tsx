import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props { children: ReactNode }
interface State { error: Error | null }

/**
 * Catches render errors that would otherwise leave the cashier staring at a
 * blank window, and offers a way back instead of forcing a restart.
 */
export default class ErrorBoundary extends Component<Props, State> {
    state: State = { error: null };

    static getDerivedStateFromError(error: Error): State {
        return { error };
    }

    componentDidCatch(error: Error, info: ErrorInfo) {
        // Kept as console output: it is the only channel the webview has, and
        // Tauri forwards it to the app log in a dev build.
        console.error('Error no controlado en la interfaz:', error, info.componentStack);
    }

    render() {
        const { error } = this.state;
        if (!error) return this.props.children;

        return (
            <div style={{
                minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
                padding: 24, background: 'var(--bg, #000000)', color: 'var(--t1, #f4f4f5)',
            }}>
                <div className="card" style={{ maxWidth: 560, width: '100%', padding: 28 }}>
                    <h1 style={{ fontSize: 20, fontWeight: 800, marginBottom: 8 }}>
                        Algo salió mal
                    </h1>
                    <p style={{ fontSize: 13, color: 'var(--t3, #6e6e76)', marginBottom: 16 }}>
                        La pantalla no pudo dibujarse. Tus datos están a salvo: nada se
                        guardó a medias.
                    </p>
                    <pre style={{
                        fontSize: 11, lineHeight: 1.5, padding: 12, borderRadius: 10,
                        background: 'rgba(255,255,255,.04)', color: 'var(--danger, #f45270)',
                        overflowX: 'auto', marginBottom: 20, whiteSpace: 'pre-wrap',
                    }}>{error.message}</pre>
                    <div style={{ display: 'flex', gap: 10 }}>
                        <button className="btn btn-primary" onClick={() => { window.location.hash = '#/'; window.location.reload(); }}>
                            Volver al inicio
                        </button>
                        <button className="btn" onClick={() => this.setState({ error: null })}>
                            Reintentar
                        </button>
                    </div>
                </div>
            </div>
        );
    }
}
