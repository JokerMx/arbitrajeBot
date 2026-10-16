import React from "react";
import { useConfigStore } from "../../store/configStore";
import { backendApiBaseUrl } from "../../services/backendConfig";
import { Save, RotateCcw, MessageSquare, Send } from "lucide-react";
import toast from "react-hot-toast";

export const TelegramSettings: React.FC = () => {
  const { config, updateTelegram } = useConfigStore();
  const [localTelegram, setLocalTelegram] = React.useState(config?.telegram);
  const [isEditing, setIsEditing] = React.useState(false);

  React.useEffect(() => {
    if (config) {
      setLocalTelegram(config.telegram);
    }
  }, [config]);

  if (!localTelegram) return null;

  const handleChange = (field: keyof typeof localTelegram, value: any) => {
    setLocalTelegram({ ...localTelegram, [field]: value });
    setIsEditing(true);
  };

  const handleSave = async () => {
    try {
      await updateTelegram(localTelegram);
      setIsEditing(false);
      toast.success("Guardado. Reinicia el bot (botón de encendido arriba) para aplicar token o chat.", { duration: 6000 });
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    if (config) {
      setLocalTelegram(config.telegram);
      setIsEditing(false);
    }
  };

  const handleTestNotification = async () => {
    toast.loading("Enviando notificación de prueba...", { id: "test" });
    try {
      // Solo al chat del administrador (antes enviaba a TODOS los usuarios)
      const response = await fetch(`${backendApiBaseUrl}/telegram/test`, { method: "POST" });
      const data = await response.json();
      if (data.success) {
        toast.success(data.message || "Notificación enviada", { id: "test" });
      } else {
        toast.error(data.message || "Error al enviar", { id: "test" });
      }
    } catch (error) {
      toast.error("Error de conexión", { id: "test" });
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Configuración de Telegram</h3>
        <MessageSquare className="w-5 h-5 text-primary-500" />
      </div>

      <div className="space-y-4">
        <div className="flex items-center justify-between p-3 bg-dark-300 rounded-lg">
          <div>
            <label className="text-sm text-gray-300">Notificaciones Activas</label>
            <p className="text-xs text-gray-500">Habilitar/deshabilitar todas las notificaciones</p>
          </div>
          <button
            onClick={() => handleChange("enabled", !localTelegram.enabled)}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              localTelegram.enabled ? "bg-success" : "bg-dark-400"
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                localTelegram.enabled ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Bot Token
          </label>
          <input
            type="password"
            value={localTelegram.botToken}
            onChange={(e) => handleChange("botToken", e.target.value)}
            placeholder="Ingresa el token del bot"
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400 placeholder-gray-600"
          />
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Chat ID
          </label>
          <input
            type="text"
            value={localTelegram.chatId}
            onChange={(e) => handleChange("chatId", e.target.value)}
            placeholder="Ingresa el chat ID"
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400 placeholder-gray-600"
          />
        </div>

        <div className="border-t border-dark-300 pt-4">
          <p className="text-sm text-gray-400 mb-3">Notificaciones</p>
          
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Send className="w-4 h-4 text-gray-500" />
                <label className="text-sm text-gray-300">Órdenes</label>
              </div>
              <button
                onClick={() => handleChange("notifyOrders", !localTelegram.notifyOrders)}
                className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                  localTelegram.notifyOrders ? "bg-primary-600" : "bg-dark-400"
                }`}
              >
                <span
                  className={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform ${
                    localTelegram.notifyOrders ? "translate-x-5" : "translate-x-1"
                  }`}
                />
              </button>
            </div>

            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Send className="w-4 h-4 text-gray-500" />
                <label className="text-sm text-gray-300">Fills (Ejecuciones)</label>
              </div>
              <button
                onClick={() => handleChange("notifyFills", !localTelegram.notifyFills)}
                className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                  localTelegram.notifyFills ? "bg-primary-600" : "bg-dark-400"
                }`}
              >
                <span
                  className={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform ${
                    localTelegram.notifyFills ? "translate-x-5" : "translate-x-1"
                  }`}
                />
              </button>
            </div>

            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Send className="w-4 h-4 text-gray-500" />
                <label className="text-sm text-gray-300">Estado del Bot</label>
              </div>
              <button
                onClick={() => handleChange("notifyStatus", !localTelegram.notifyStatus)}
                className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                  localTelegram.notifyStatus ? "bg-primary-600" : "bg-dark-400"
                }`}
              >
                <span
                  className={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform ${
                    localTelegram.notifyStatus ? "translate-x-5" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
          </div>
        </div>

        <button
          onClick={handleTestNotification}
          className="w-full flex items-center justify-center gap-2 bg-dark-300 hover:bg-dark-400 text-gray-300 rounded-lg px-4 py-2 text-sm transition-colors"
        >
          <Send className="w-4 h-4" />
          Enviar Notificación de Prueba
        </button>

        {isEditing && (
          <div className="flex gap-2 pt-4 border-t border-dark-300">
            <button
              onClick={handleSave}
              className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg px-4 py-2 text-sm transition-colors"
            >
              <Save className="w-4 h-4" />
              Guardar Cambios
            </button>
            <button
              onClick={handleReset}
              className="flex items-center justify-center gap-2 bg-dark-300 hover:bg-dark-400 text-gray-300 rounded-lg px-4 py-2 text-sm transition-colors"
            >
              <RotateCcw className="w-4 h-4" />
              Cancelar
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
