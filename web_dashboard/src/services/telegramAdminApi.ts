import axios from 'axios';
import { TelegramUser, TelegramStats, TelegramCommand } from '../types/telegram';
import toast from 'react-hot-toast';
import { backendApiBaseUrl } from './backendConfig';

const API_BASE = backendApiBaseUrl;

class TelegramAdminApi {
  private async request<T>(
    method: 'get' | 'post' | 'put' | 'delete',
    endpoint: string,
    data?: any
  ): Promise<T> {
    try {
      const response = await axios({
        method,
        url: `${API_BASE}/telegram${endpoint}`,
        data,
        headers: {
          'Content-Type': 'application/json',
        },
      });
      return response.data;
    } catch (error) {
      console.error('Error en petición Telegram:', error);
      throw error;
    }
  }

  // Obtener lista de usuarios
  async getUsers(): Promise<TelegramUser[]> {
    return this.request('get', '/users');
  }

  // Obtener estadísticas
  async getStats(): Promise<TelegramStats> {
    return this.request('get', '/stats');
  }

  // Autorizar un usuario
  async authorizeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/authorize`);
  }

  // Desautorizar un usuario
  async deauthorizeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/deauthorize`);
  }

  // Suscribir un usuario
  async subscribeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/subscribe`);
  }

  // Desuscribir un usuario
  async unsubscribeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/unsubscribe`);
  }

  // Eliminar un usuario
  async deleteUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('delete', `/users/${chatId}`);
  }

  // Enviar broadcast
  async sendBroadcast(message: string): Promise<{ success: boolean; message: string; sent: number }> {
    return this.request('post', '/broadcast', { message });
  }

  // Obtener comandos
  async getCommands(): Promise<TelegramCommand[]> {
    return this.request('get', '/commands');
  }
}

export const telegramApi = new TelegramAdminApi();