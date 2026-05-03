import { useState, useEffect, useRef } from 'react';
import * as api from '../../api';
import type { Notification } from '../../types';
import { useNavigate } from 'react-router-dom';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoBell     = ({ size = 15 }: { size?: number }) => <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>;
const IcoX        = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoPackage  = () => <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>;
const IcoCalendar = () => <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>;

export default function NotificationBell() {
    const [notifications, setNotifications] = useState<Notification[]>([]);
    const [isOpen, setIsOpen] = useState(false);
    const popoverRef = useRef<HTMLDivElement>(null);
    const navigate = useNavigate();

    const loadNotifications = async () => {
        try { const data = await api.getNotifications(); setNotifications(data); }
        catch { /* silent */ }
    };

    useEffect(() => {
        loadNotifications();
        const interval = setInterval(loadNotifications, 60000);
        return () => clearInterval(interval);
    }, []);

    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) setIsOpen(false);
        };
        if (isOpen) document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, [isOpen]);

    const handleDismiss = async (id: number, type: string) => {
        try {
            await api.markNotificationRead(id, type);
            setNotifications(prev => prev.filter(n => !(n.id === id && n.notification_type === type)));
        } catch { /* silent */ }
    };

    const handleViewProduct = (productId: number | null) => {
        setIsOpen(false);
        if (productId) navigate(`/products?search=${productId}`);
    };

    const unreadCount = notifications.length;

    return (
        <div className="notif-wrap" ref={popoverRef}>
            <button
                onClick={() => setIsOpen(!isOpen)}
                className={`notif-bell-btn${isOpen ? ' open' : ''}`}
            >
                <IcoBell />
                {unreadCount > 0 && (
                    <span className="notif-badge">
                        {unreadCount > 9 ? '9+' : unreadCount}
                    </span>
                )}
            </button>

            {isOpen && (
                <div className="notif-popover animate-scale-in">
                    <div className="notif-popover-header">
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <span style={{ color: 'var(--primary)' }}><IcoBell size={13} /></span>
                            <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)' }}>Notificaciones</span>
                            {unreadCount > 0 && <span className="badge badge-primary">{unreadCount}</span>}
                        </div>
                        <button onClick={() => setIsOpen(false)} className="notif-close-btn">
                            <IcoX />
                        </button>
                    </div>

                    <div className="notif-popover-body">
                        {notifications.length === 0 ? (
                            <div style={{ padding: '32px 16px', textAlign: 'center', color: 'var(--t3)' }}>
                                <IcoBell size={24} />
                                <p style={{ fontSize: 12, marginTop: 8 }}>Sin notificaciones pendientes</p>
                            </div>
                        ) : (
                            notifications.map(notif => (
                                <div key={`${notif.notification_type}-${notif.id}`} className="notif-item">
                                    <div
                                        className="notif-item-accent"
                                        style={{ background: notif.notification_type === 'low_stock' ? 'var(--danger)' : 'var(--primary)' }}
                                    />
                                    <div
                                        className="notif-item-icon"
                                        style={{
                                            background: notif.notification_type === 'low_stock' ? 'rgba(244,82,112,0.1)' : 'rgba(139,120,245,0.1)',
                                            color: notif.notification_type === 'low_stock' ? 'var(--danger)' : 'var(--primary)',
                                        }}
                                    >
                                        {notif.notification_type === 'low_stock' ? <IcoPackage /> : <IcoCalendar />}
                                    </div>
                                    <div style={{ flex: 1, minWidth: 0 }}>
                                        <p style={{ fontSize: 12, fontWeight: 500, color: 'var(--t1)', lineHeight: 1.5 }}>
                                            {notif.message}
                                        </p>
                                        {notif.target_date && (
                                            <p style={{ fontSize: 11, color: 'var(--primary)', fontFamily: 'monospace', marginTop: 2 }}>
                                                Para: {notif.target_date}
                                            </p>
                                        )}
                                        <div style={{ display: 'flex', gap: 12, marginTop: 6 }}>
                                            <button
                                                onClick={() => handleDismiss(notif.id, notif.notification_type)}
                                                style={{ fontSize: 11, color: 'var(--t3)' }}
                                                onMouseEnter={e => (e.currentTarget as HTMLButtonElement).style.color = 'var(--t2)'}
                                                onMouseLeave={e => (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'}
                                            >
                                                Ignorar
                                            </button>
                                            {notif.product_id && (
                                                <button
                                                    onClick={() => handleViewProduct(notif.product_id)}
                                                    style={{ fontSize: 11, fontWeight: 600, color: 'var(--primary)' }}
                                                    onMouseEnter={e => (e.currentTarget as HTMLButtonElement).style.color = 'var(--primary-d)'}
                                                    onMouseLeave={e => (e.currentTarget as HTMLButtonElement).style.color = 'var(--primary)'}
                                                >
                                                    Ver producto →
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
