export interface TelegramUser {
  chat_id: string;
  username: string;
  first_name: string;
  subscribed: boolean;
  authorized: boolean;
  last_interaction: string;
  is_admin?: boolean;
  premium_until?: string | null;
  is_premium?: boolean;
  trial_used?: boolean;
  referred_by?: string | null;
  referral_count?: number;
  blocked?: boolean;
  total_paid_stars?: number;
  joined_at?: string;
}

export interface TelegramStats {
  total_users: number;
  authorized_users: number;
  subscribed_users: number;
  active_today: number;
  premium_users?: number;
  paying_users?: number;
  blocked_users?: number;
  new_users_7d?: number;
  revenue_stars_30d?: number;
  revenue_stars_total?: number;
  premium_price_stars?: number;
}

export interface PaymentRecord {
  chat_id: string;
  stars: number;
  charge_id: string;
  payload: string;
  recurring: boolean;
  at: string;
  premium_until?: string | null;
}

export interface SignalPerformance {
  closed: number;
  wins: number;
  win_rate: number;
  avg_win_pct: number;
  avg_loss_pct: number;
  total_r: number;
  best_pct: number;
  worst_pct: number;
  open: number;
  includes_paper: boolean;
  since?: string | null;
}

export type BroadcastSegment = 'all' | 'free' | 'premium';

export interface TelegramCommand {
  command: string;
  description: string;
  access: 'public' | 'authorized' | 'premium' | 'admin';
}