export interface TelegramUser {
  chat_id: string;
  username: string;
  first_name: string;
  subscribed: boolean;
  authorized: boolean;
  last_interaction: string;
  is_admin?: boolean;
}

export interface TelegramStats {
  total_users: number;
  authorized_users: number;
  subscribed_users: number;
  active_today: number;
}

export interface TelegramCommand {
  command: string;
  description: string;
  access: 'public' | 'authorized' | 'admin';
}