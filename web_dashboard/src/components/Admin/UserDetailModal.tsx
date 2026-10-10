import React from 'react';
import { X, Check, XCircle, Bell, BellOff, UserCheck, UserX } from 'lucide-react';
import { TelegramUser } from '../../types/telegram';
import { format } from 'date-fns';
import { es } from 'date-fns/locale';

interface UserDetailModalProps {
  user: TelegramUser | null;
  isOpen: boolean;
  onClose: () => void;
  onAuthorize: (chatId: string) => void;
  onDeauthorize: (chatId: string) => void;
  onSubscribe: (chatId: string) => void;
  onUnsubscribe: (chatId: string) => void;
  onDelete: (chatId: string) => void;
}

export const UserDetailModal: React.FC<UserDetailModalProps> = ({
  user,
  isOpen,
  onClose,
  onAuthorize,
  onDeauthorize,
  onSubscribe,
  onUnsubscribe,
  onDelete,
}) => {
  if (!isOpen || !user) return null;

  const lastInteraction = user.last_interaction 
    ? format(new Date(user.last_interaction), "dd/MM/yyyy HH:mm:ss", { locale: es })
    : 'Desconocido';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Overlay */}
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" onClick={onClose} />
      
      {/* Modal */}
      <div className="relative bg-dark-200 rounded-xl border border-dark-300 shadow-2xl w-full max-w-lg">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-dark-300">
          <h2 className="text-xl font-bold">Detalles del Usuario</h2>
          <button
            onClick={onClose}
            className="p-2 hover:bg-dark-300 rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6">
          {/* Información básica */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <p className="text-sm text-gray-400">Chat ID</p>
              <p className="font-mono text-sm bg-dark-300 p-2 rounded-lg mt-1">
                {user.chat_id}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-400">Username</p>
              <p className="font-medium">@{user.username}</p>
            </div>
            <div>
              <p className="text-sm text-gray-400">Nombre</p>
              <p className="font-medium">{user.first_name}</p>
            </div>
            <div>
              <p className="text-sm text-gray-400">Última interacción</p>
              <p className="text-sm">{lastInteraction}</p>
            </div>
            <div>
              <p className="text-sm text-gray-400">Plan</p>
              <p className="font-medium">
                {user.is_premium
                  ? `💎 Premium hasta ${user.premium_until ? new Date(user.premium_until).toLocaleDateString() : '—'}`
                  : `Free${user.trial_used ? ' (prueba usada)' : ''}`}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-400">Pagos / referidos</p>
              <p className="text-sm">
                {user.total_paid_stars || 0} ⭐ · {user.referral_count || 0} referidos
                {user.referred_by ? ` · invitado por ${user.referred_by}` : ''}
                {user.blocked ? ' · bloqueó el bot' : ''}
              </p>
            </div>
          </div>

          {/* Estado actual */}
          <div className="grid grid-cols-2 gap-4">
            <div className={`p-4 rounded-lg border ${
              user.authorized 
                ? 'bg-success/10 border-success/20' 
                : 'bg-danger/10 border-danger/20'
            }`}>
              <p className="text-sm text-gray-400 mb-1">Autorización</p>
              <div className="flex items-center gap-2">
                {user.authorized ? (
                  <>
                    <UserCheck className="w-5 h-5 text-success" />
                    <span className="font-medium text-success">Autorizado</span>
                  </>
                ) : (
                  <>
                    <UserX className="w-5 h-5 text-danger" />
                    <span className="font-medium text-danger">No autorizado</span>
                  </>
                )}
              </div>
            </div>

            <div className={`p-4 rounded-lg border ${
              user.subscribed 
                ? 'bg-primary-500/10 border-primary-500/20' 
                : 'bg-dark-300/50 border-dark-400'
            }`}>
              <p className="text-sm text-gray-400 mb-1">Suscripción</p>
              <div className="flex items-center gap-2">
                {user.subscribed ? (
                  <>
                    <Bell className="w-5 h-5 text-primary-500" />
                    <span className="font-medium text-primary-500">Suscrito</span>
                  </>
                ) : (
                  <>
                    <BellOff className="w-5 h-5 text-gray-400" />
                    <span className="font-medium text-gray-400">No suscrito</span>
                  </>
                )}
              </div>
            </div>
          </div>

          {/* Acciones */}
          <div className="space-y-3">
            <h3 className="font-semibold">Acciones</h3>
            <div className="grid grid-cols-2 gap-3">
              {!user.authorized ? (
                <button
                  onClick={() => onAuthorize(user.chat_id)}
                  className="flex items-center justify-center gap-2 bg-success/20 hover:bg-success/30 text-success border border-success/30 rounded-lg px-4 py-2 transition-colors"
                >
                  <UserCheck className="w-4 h-4" />
                  Autorizar
                </button>
              ) : (
                <button
                  onClick={() => onDeauthorize(user.chat_id)}
                  className="flex items-center justify-center gap-2 bg-warning/20 hover:bg-warning/30 text-warning border border-warning/30 rounded-lg px-4 py-2 transition-colors"
                >
                  <UserX className="w-4 h-4" />
                  Desautorizar
                </button>
              )}

              {!user.subscribed ? (
                <button
                  onClick={() => onSubscribe(user.chat_id)}
                  className="flex items-center justify-center gap-2 bg-primary-500/20 hover:bg-primary-500/30 text-primary-500 border border-primary-500/30 rounded-lg px-4 py-2 transition-colors"
                >
                  <Bell className="w-4 h-4" />
                  Suscribir
                </button>
              ) : (
                <button
                  onClick={() => onUnsubscribe(user.chat_id)}
                  className="flex items-center justify-center gap-2 bg-dark-300 hover:bg-dark-400 text-gray-300 border border-dark-400 rounded-lg px-4 py-2 transition-colors"
                >
                  <BellOff className="w-4 h-4" />
                  Desuscribir
                </button>
              )}
            </div>

            <button
              onClick={() => {
                if (window.confirm(`¿Estás seguro de eliminar al usuario ${user.first_name}?`)) {
                  onDelete(user.chat_id);
                  onClose();
                }
              }}
              className="w-full flex items-center justify-center gap-2 bg-danger/20 hover:bg-danger/30 text-danger border border-danger/30 rounded-lg px-4 py-2 transition-colors mt-2"
            >
              <XCircle className="w-4 h-4" />
              Eliminar Usuario
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};