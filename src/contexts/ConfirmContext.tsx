import { createContext, useContext, useState, useCallback } from 'react';

const IcoAlert = () => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>;
const IcoX    = () => <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;

export interface ConfirmOptions {
    title?: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: 'danger' | 'warning';
}

interface ConfirmContextValue {
    confirm: (options: ConfirmOptions | string) => Promise<boolean>;
}

const ConfirmContext = createContext<ConfirmContextValue>({ confirm: () => Promise.resolve(false) });

export function useConfirm() {
    return useContext(ConfirmContext);
}

interface Pending {
    options: ConfirmOptions;
    resolve: (v: boolean) => void;
}

export function ConfirmProvider({ children }: { children: React.ReactNode }) {
    const [pending, setPending] = useState<Pending | null>(null);

    const confirm = useCallback((options: ConfirmOptions | string): Promise<boolean> => {
        const opts = typeof options === 'string' ? { message: options } : options;
        return new Promise(resolve => setPending({ options: opts, resolve }));
    }, []);

    const respond = (value: boolean) => {
        pending?.resolve(value);
        setPending(null);
    };

    const isDanger = pending?.options.variant === 'danger';

    return (
        <ConfirmContext.Provider value={{ confirm }}>
            {children}
            {pending && (
                <div
                    className="modal-overlay"
                    style={{ zIndex: 10000 }}
                    onClick={() => respond(false)}
                >
                    <div
                        className="glass-modal animate-scale-in"
                        style={{ width: '100%', maxWidth: 400, padding: 0, overflow: 'hidden' }}
                        onClick={e => e.stopPropagation()}
                    >
                        {/* Header */}
                        <div style={{ padding: '24px 24px 0', display: 'flex', alignItems: 'flex-start', gap: 14 }}>
                            <div style={{
                                width: 42, height: 42, borderRadius: 10, flexShrink: 0,
                                display: 'flex', alignItems: 'center', justifyContent: 'center',
                                background: isDanger ? 'rgba(244,63,94,0.1)' : 'rgba(245,158,11,0.1)',
                            }}>
                                <span style={{ color: isDanger ? 'var(--danger)' : 'var(--warning)' }}><IcoAlert /></span>
                            </div>
                            <div style={{ flex: 1, minWidth: 0 }}>
                                <h3 style={{ fontSize: 15, fontWeight: 700, color: '#f1f3f9', lineHeight: 1.3 }}>
                                    {pending.options.title ?? 'Confirmar acción'}
                                </h3>
                                <p style={{ marginTop: 6, fontSize: 13, color: '#8d95a8', lineHeight: 1.6 }}>
                                    {pending.options.message}
                                </p>
                            </div>
                            <button
                                onClick={() => respond(false)}
                                style={{ color: 'var(--t3)', padding: 4, flexShrink: 0 }}
                            >
                                <IcoX />
                            </button>
                        </div>

                        {/* Footer */}
                        <div style={{
                            padding: '20px 24px',
                            display: 'flex',
                            justifyContent: 'flex-end',
                            gap: 8,
                            borderTop: '1px solid rgba(0,0,0,0.07)',
                            marginTop: 20,
                        }}>
                            <button onClick={() => respond(false)} className="btn btn-ghost">
                                {pending.options.cancelLabel ?? 'Cancelar'}
                            </button>
                            <button
                                onClick={() => respond(true)}
                                className={`btn ${isDanger ? 'btn-danger' : 'btn-primary'}`}
                            >
                                {pending.options.confirmLabel ?? 'Confirmar'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </ConfirmContext.Provider>
    );
}
