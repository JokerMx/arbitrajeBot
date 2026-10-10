import axios from 'axios';
import {
  TelegramUser,
  TelegramStats,
  TelegramCommand,
  PaymentRecord,
  SignalPerformance,
  BroadcastSegment,
} from '../types/telegram';
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
    } catch (error: any) {
      console.error('Error en petición Telegram:', error);
      if (error.response) {
        console.error('Respuesta del servidor:', error.response.data);
      }
      throw error;
    }
  }

  async getUsers(): Promise<TelegramUser[]> {
    return this.request('get', '/users');
  }

  async getStats(): Promise<TelegramStats> {
    return this.request('get', '/stats');
  }

  async authorizeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/authorize`);
  }

  async deauthorizeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/deauthorize`);
  }

  async subscribeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/subscribe`);
  }

  async unsubscribeUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/unsubscribe`);
  }

  async deleteUser(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('delete', `/users/${chatId}`);
  }

  async sendBroadcast(
    message: string,
    segment: BroadcastSegment = 'all'
  ): Promise<{ success: boolean; message: string; sent: number }> {
    return this.request('post', '/broadcast', { message, segment });
  }

  async grantPremium(chatId: string, days: number): Promise<{ success: boolean; message: string }> {
    return this.request('post', `/users/${chatId}/premium`, { days });
  }

  async revokePremium(chatId: string): Promise<{ success: boolean; message: string }> {
    return this.request('delete', `/users/${chatId}/premium`);
  }

  async getPayments(): Promise<PaymentRecord[]> {
    return this.request('get', '/payments');
  }

  async getPerformance(): Promise<SignalPerformance> {
    const r = await axios.get(`${API_BASE}/signals/performance`);
    return r.data;
  }

  async getCommands(): Promise<TelegramCommand[]> {
    return this.request('get', '/commands');
  }
}

export const telegramApi = new TelegramAdminApi();