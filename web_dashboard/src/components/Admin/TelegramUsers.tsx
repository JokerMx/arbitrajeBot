import React, { useState, useEffect } from 'react';
import {
  Users,
  UserCheck,
  Bell,
  BellOff,
  Activity,
  RefreshCw,
  Send,
  Search,
  MoreVertical,
  Check,
  X,
  Loader2,
  AlertCircle,
} from 'lucide-react';
import { telegramApi } from '../../services/telegramAdminApi';
import { TelegramUser, TelegramStats, TelegramCommand } from '../../types/telegram';
import { UserDetailModal } from './UserDetailModal';
import toast from 'react-hot-toast';

export const TelegramUsers: React.FC = () => {
  const [users, setUsers] = useState<TelegramUser[]>([]);
  const [stats, setStats] = useState<TelegramStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [searchTerm, setSearchTerm] = useState('');
  const [filter, setFilter] = useState<'all' | 'authorized' | 'unauthorized' | 'subscribed'>('all');
  const [selectedUser, setSelectedUser] = useState<TelegramUser | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [broadcastMessage, setBroadcastMessage] = useState('');
  const [sendingBroadcast, setSendingBroadcast] = useState(false);
  const [commands, setCommands] = useState<TelegramCommand[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Cargar datos
  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      console.log("📡 Cargando datos de Telegram...");
      const [usersData, statsData, commandsData] = await Promise.all([
        telegramApi.getUsers(),
        telegramApi.getStats(),
        telegramApi.getCommands(),
      ]);
      console.log("✅ Usuarios cargados:", usersData);
      console.log("✅ Estadísticas cargadas:", statsData);
      console.log("✅ Comandos cargados:", commandsData);
      setUsers(usersData);
      setStats(statsData);
      setCommands(commandsData);
    } catch (error) {
      console.error('❌ Error cargando datos:', error);
      setError('Error al cargar los datos. Por favor, intenta de nuevo.');
      toast.error('Error al cargar los usuarios');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    // Actualizar cada 30 segundos
    const interval = setInterval(loadData, 30000);
    return () => clearInterval(interval);
  }, []);

  // Filtrar usuarios
  const filteredUsers = users.filter(user => {
    const matchesSearch = 
      user.chat_id.includes(searchTerm) ||
      user.username.toLowerCase().includes(searchTerm.toLowerCase()) ||
      user.first_name.toLowerCase().includes(searchTerm.toLowerCase());
    
    if (!matchesSearch) return false;

    switch (filter) {
      case 'authorized':
        return user.authorized;
      case 'unauthorized':
        return !user.authorized;
      case 'subscribed':
        return user.subscribed;
      default:
        return true;
    }
  });

  // Acciones
  const handleAuthorize = async (chatId: string) => {
    try {
      const result = await telegramApi.authorizeUser(chatId);
      if (result.success) {
        toast.success(result.message);
        loadData(); // Recargar datos
      }
    } catch (error) {
      toast.error('Error al autorizar usuario');
    }
  };

  const handleDeauthorize = async (chatId: string) => {
    try {
      const result = await telegramApi.deauthorizeUser(chatId);
      if (result.success) {
        toast.success(result.message);
        loadData();
      }
    } catch (error) {
      toast.error('Error al desautorizar usuario');
    }
  };

  const handleSubscribe = async (chatId: string) => {
    try {
      const result = await telegramApi.subscribeUser(chatId);
      if (result.success) {
        toast.success(result.message);
        loadData();
      } else {
        toast.error(result.message);
      }
    } catch (error) {
      toast.error('Error al suscribir usuario');
    }
  };

  const handleUnsubscribe = async (chatId: string) => {
    try {
      const result = await telegramApi.unsubscribeUser(chatId);
      if (result.success) {
        toast.success(result.message);
        loadData();
      }
    } catch (error) {
      toast.error('Error al desuscribir usuario');
    }
  };

  const handleDelete = async (chatId: string) => {
    try {
      const result = await telegramApi.deleteUser(chatId);
      if (result.success) {
        toast.success(result.message);
        loadData();
      }
    } catch (error) {
      toast.error('Error al eliminar usuario');
    }
  };

  const handleBroadcast = async () => {
    if (!broadcastMessage.trim()) {
      toast.error('El mensaje no puede estar vacío');
      return;
    }

    setSendingBroadcast(true);
    try {
      const result = await telegramApi.sendBroadcast(broadcastMessage);
      if (result.success) {
        toast.success(`Mensaje enviado a ${result.sent} usuarios`);
        setBroadcastMessage('');
      }
    } catch (error) {
      toast.error('Error al enviar broadcast');
    } finally {
      setSendingBroadcast(false);
    }
  };

  if (loading && users.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="w-8 h-8 animate-spin text-primary-500" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-dark-200 rounded-xl p-8 border border-dark-300 text-center">
        <AlertCircle className="w-12 h-12 text-danger mx-auto mb-4" />
        <h3 className="text-xl font-bold mb-2">Error al cargar datos</h3>
        <p className="text-gray-400 mb-4">{error}</p>
        <button
          onClick={loadData}
          className="bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-lg transition-colors"
        >
          Intentar de nuevo
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header con estadísticas */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-primary-500/10 rounded-lg">
              <Users className="w-6 h-6 text-primary-500" />
            </div>
            <div>
              <p className="text-sm text-gray-400">Total Usuarios</p>
              <p className="text-2xl font-bold">{stats?.total_users || 0}</p>
            </div>
          </div>
        </div>

        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-success/10 rounded-lg">
              <UserCheck className="w-6 h-6 text-success" />
            </div>
            <div>
              <p className="text-sm text-gray-400">Autorizados</p>
              <p className="text-2xl font-bold">{stats?.authorized_users || 0}</p>
            </div>
          </div>
        </div>

        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-warning/10 rounded-lg">
              <Bell className="w-6 h-6 text-warning" />
            </div>
            <div>
              <p className="text-sm text-gray-400">Suscritos</p>
              <p className="text-2xl font-bold">{stats?.subscribed_users || 0}</p>
            </div>
          </div>
        </div>

        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <div className="flex items-center gap-3">
            <div className="p-3 bg-info/10 rounded-lg">
              <Activity className="w-6 h-6 text-info" />
            </div>
            <div>
              <p className="text-sm text-gray-400">Activos Hoy</p>
              <p className="text-2xl font-bold">{stats?.active_today || 0}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Broadcast Section */}
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Enviar Broadcast</h3>
        <div className="flex gap-3">
          <input
            type="text"
            value={broadcastMessage}
            onChange={(e) => setBroadcastMessage(e.target.value)}
            placeholder="Escribe un mensaje para enviar a todos los usuarios suscritos..."
            className="flex-1 bg-dark-300 border border-dark-400 rounded-lg px-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-500"
            disabled={sendingBroadcast}
          />
          <button
            onClick={handleBroadcast}
            disabled={sendingBroadcast || !broadcastMessage.trim()}
            className="flex items-center gap-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg px-6 py-2 transition-colors"
          >
            {sendingBroadcast ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <Send className="w-4 h-4" />
            )}
            Enviar
          </button>
        </div>
      </div>

      {/* Filtros y búsqueda */}
      <div className="flex flex-wrap gap-4 items-center justify-between">
        <div className="flex gap-2">
          <button
            onClick={() => setFilter('all')}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              filter === 'all'
                ? 'bg-primary-600 text-white'
                : 'bg-dark-300 text-gray-400 hover:bg-dark-400'
            }`}
          >
            Todos ({users.length})
          </button>
          <button
            onClick={() => setFilter('authorized')}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              filter === 'authorized'
                ? 'bg-success text-white'
                : 'bg-dark-300 text-gray-400 hover:bg-dark-400'
            }`}
          >
            Autorizados ({users.filter(u => u.authorized).length})
          </button>
          <button
            onClick={() => setFilter('unauthorized')}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              filter === 'unauthorized'
                ? 'bg-danger text-white'
                : 'bg-dark-300 text-gray-400 hover:bg-dark-400'
            }`}
          >
            No autorizados ({users.filter(u => !u.authorized).length})
          </button>
          <button
            onClick={() => setFilter('subscribed')}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              filter === 'subscribed'
                ? 'bg-warning text-white'
                : 'bg-dark-300 text-gray-400 hover:bg-dark-400'
            }`}
          >
            Suscritos ({users.filter(u => u.subscribed).length})
          </button>
        </div>

        <div className="flex gap-2">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Buscar usuario..."
              className="bg-dark-300 border border-dark-400 rounded-lg pl-9 pr-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-500"
            />
          </div>
          <button
            onClick={loadData}
            className="p-2 bg-dark-300 hover:bg-dark-400 rounded-lg transition-colors"
            title="Actualizar"
          >
            <RefreshCw className={`w-5 h-5 ${loading ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>

      {/* Tabla de usuarios */}
      <div className="bg-dark-200 rounded-xl border border-dark-300 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-dark-300">
              <tr className="text-left text-sm text-gray-400">
                <th className="px-6 py-4">Usuario</th>
                <th className="px-6 py-4">Chat ID</th>
                <th className="px-6 py-4">Estado</th>
                <th className="px-6 py-4">Suscripción</th>
                <th className="px-6 py-4">Última interacción</th>
                <th className="px-6 py-4">Acciones</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-dark-300">
              {filteredUsers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-6 py-8 text-center text-gray-400">
                    No hay usuarios que coincidan con los filtros
                  </td>
                </tr>
              ) : (
                filteredUsers.map((user) => (
                  <tr key={user.chat_id} className="hover:bg-dark-300/50 transition-colors">
                    <td className="px-6 py-4">
                      <div>
                        <p className="font-medium">{user.first_name}</p>
                        <p className="text-sm text-gray-400">@{user.username}</p>
                      </div>
                    </td>
                    <td className="px-6 py-4 font-mono text-sm">{user.chat_id}</td>
                    <td className="px-6 py-4">
                      {user.authorized ? (
                        <span className="flex items-center gap-1 text-success text-sm">
                          <Check className="w-4 h-4" />
                          Autorizado
                        </span>
                      ) : (
                        <span className="flex items-center gap-1 text-danger text-sm">
                          <X className="w-4 h-4" />
                          No autorizado
                        </span>
                      )}
                    </td>
                    <td className="px-6 py-4">
                      {user.subscribed ? (
                        <span className="flex items-center gap-1 text-primary-500 text-sm">
                          <Bell className="w-4 h-4" />
                          Suscrito
                        </span>
                      ) : (
                        <span className="flex items-center gap-1 text-gray-400 text-sm">
                          <BellOff className="w-4 h-4" />
                          No suscrito
                        </span>
                      )}
                    </td>
                    <td className="px-6 py-4 text-sm text-gray-400">
                      {new Date(user.last_interaction).toLocaleString()}
                    </td>
                    <td className="px-6 py-4">
                      <button
                        onClick={() => {
                          setSelectedUser(user);
                          setModalOpen(true);
                        }}
                        className="p-2 hover:bg-dark-300 rounded-lg transition-colors"
                        title="Ver detalles"
                      >
                        <MoreVertical className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Comandos disponibles */}
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Comandos de Telegram</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {commands.map((cmd, index) => (
            <div key={index} className="bg-dark-300/50 rounded-lg p-4 border border-dark-400">
              <div className="flex items-center gap-2 mb-2">
                <code className="bg-dark-400 px-2 py-1 rounded text-primary-400 font-mono">
                  {cmd.command}
                </code>
                <span className={`text-xs px-2 py-1 rounded-full ${
                  cmd.access === 'admin' ? 'bg-danger/20 text-danger' :
                  cmd.access === 'authorized' ? 'bg-success/20 text-success' :
                  'bg-primary-500/20 text-primary-500'
                }`}>
                  {cmd.access}
                </span>
              </div>
              <p className="text-sm text-gray-400">{cmd.description}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Modal de detalles */}
      <UserDetailModal
        user={selectedUser}
        isOpen={modalOpen}
        onClose={() => setModalOpen(false)}
        onAuthorize={handleAuthorize}
        onDeauthorize={handleDeauthorize}
        onSubscribe={handleSubscribe}
        onUnsubscribe={handleUnsubscribe}
        onDelete={handleDelete}
      />
    </div>
  );
};