import { useState, useEffect, useRef } from 'react';
import { Bell, X, Package, Calendar } from 'lucide-react';
import * as api from '../../api';
import type { Notification } from '../../types';
import { useNavigate } from 'react-router-dom';

export default function NotificationBell() {
    const [notifications, setNotifications] = useState<Notification[]>([]);
    const [isOpen, setIsOpen] = useState(false);
    const popoverRef = useRef<HTMLDivElement>(null);
    const navigate = useNavigate();

    const loadNotifications = async () => {
        try {
            const data = await api.getNotifications();
            setNotifications(data);
        } catch (error) {
            console.error('Failed to load notifications', error);
        }
    };

    useEffect(() => {
        loadNotifications();
        // Polling every 1 minute
        const interval = setInterval(loadNotifications, 60000);
        return () => clearInterval(interval);
    }, []);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (popoverRef.current && !popoverRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        if (isOpen) {
            document.addEventListener('mousedown', handleClickOutside);
        }
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isOpen]);

    const handleDismiss = async (id: number, type: string) => {
        try {
            await api.markNotificationRead(id, type);
            setNotifications((prev) => prev.filter((n) => !(n.id === id && n.notification_type === type)));
        } catch (error) {
            console.error('Failed to dismiss notification', error);
        }
    };

    const handleViewProduct = (productId: number | null) => {
        setIsOpen(false);
        if (productId) {
            navigate(`/products?search=${productId}`); // Assuming search accepts ID or we just go to products
        }
    };

    const unreadCount = notifications.length;

    return (
        <div className="relative" ref={popoverRef}>
            <button
                onClick={() => setIsOpen(!isOpen)}
                className="relative p-3 rounded-2xl bg-bg-secondary border border-border hover:bg-surface-hover hover:border-primary/30 transition-all group shadow-[0_4px_15px_rgba(0,0,0,0.3)]"
            >
                <Bell size={24} className="text-text-muted group-hover:text-primary transition-colors" />
                {unreadCount > 0 && (
                    <span className="absolute -top-1.5 -right-1.5 flex h-6 w-6 items-center justify-center rounded-full bg-danger text-[11px] font-black tracking-tighter text-white shadow-[0_0_12px_rgba(244,63,94,0.6)] animate-pulse-glow">
                        {unreadCount > 9 ? '9+' : unreadCount}
                    </span>
                )}
            </button>

            {isOpen && (
                <div className="absolute bottom-full right-0 mb-4 w-96 max-h-[32rem] overflow-y-auto bg-[#0B0B0F]/95 backdrop-blur-2xl border border-border rounded-3xl shadow-[0_20px_60px_rgba(0,0,0,0.8)] z-50 animate-fade-in flex flex-col">
                    <div className="flex items-center justify-between p-6 border-b border-border/50 sticky top-0 bg-[#0B0B0F]/95 backdrop-blur-2xl z-10">
                        <h3 className="text-xl font-bold tracking-tight text-white flex items-center gap-3">
                            <span className="w-2 h-2 rounded-full bg-primary animate-pulse"></span>
                            Notificaciones
                        </h3>
                        <button onClick={() => setIsOpen(false)} className="p-2 text-text-muted hover:text-white rounded-xl hover:bg-white/10 transition-colors">
                            <X size={20} />
                        </button>
                    </div>

                    <div className="p-4 space-y-3">
                        {notifications.length === 0 ? (
                            <div className="py-12 text-center flex flex-col items-center">
                                <Bell size={48} className="text-text-muted/30 mb-4" />
                                <p className="text-text-secondary font-medium">No tienes notificaciones pendientes.</p>
                            </div>
                        ) : (
                            notifications.map((notif) => (
                                <div key={`${notif.notification_type}-${notif.id}`} className="bg-bg-primary/50 border border-white/5 rounded-2xl p-5 hover:bg-bg-secondary transition-colors group relative overflow-hidden">
                                    {/* Accent strip based on type */}
                                    <div className={`absolute left-0 top-0 bottom-0 w-1 ${notif.notification_type === 'low_stock' ? 'bg-danger' : 'bg-primary'}`}></div>

                                    <div className="flex items-start gap-4 ml-2">
                                        <div className={`p-3 rounded-xl ${notif.notification_type === 'low_stock' ? 'bg-danger/10 text-danger' : 'bg-primary/10 text-primary'}`}>
                                            {notif.notification_type === 'low_stock' ? <Package size={20} /> : <Calendar size={20} />}
                                        </div>
                                        <div className="flex-1">
                                            <p className="text-sm font-semibold text-white leading-relaxed">{notif.message}</p>
                                            {notif.target_date && (
                                                <p className="text-xs text-primary font-mono mt-1.5">
                                                    Para: {notif.target_date}
                                                </p>
                                            )}
                                            <div className="flex items-center gap-3 mt-4">
                                                <button
                                                    onClick={() => handleDismiss(notif.id, notif.notification_type)}
                                                    className="text-xs font-bold text-text-muted hover:text-white transition-colors uppercase tracking-wider"
                                                >
                                                    Ignorar
                                                </button>
                                                {notif.product_id && (
                                                    <button
                                                        onClick={() => handleViewProduct(notif.product_id)}
                                                        className="text-xs font-bold text-primary hover:text-primary-hover transition-colors uppercase tracking-wider border border-primary/20 px-3 py-1.5 rounded-lg hover:bg-primary/10"
                                                    >
                                                        Ver Producto
                                                    </button>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                    <p className="absolute top-4 right-4 text-[10px] text-text-muted/50 font-mono hidden group-hover:block">{new Date(notif.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</p>
                                </div>
                            ))
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
