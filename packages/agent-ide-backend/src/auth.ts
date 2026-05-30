import * as crypto from 'crypto';
import type { Request, Response, NextFunction } from 'express';

export interface AuthUser {
    userId: string;
    email:  string;
    name:   string;
}

export interface AuthedRequest extends Request {
    user: AuthUser;
}

const JWT_SECRET    = process.env.JWT_SECRET    ?? 'agent-ide-dev-secret-CHANGE-IN-PROD';
const AUTH_ENABLED  = process.env.AUTH_ENABLED  === 'true';
const DEMO_EMAIL    = process.env.DEMO_EMAIL    ?? 'demo@agent-ide.local';
const DEMO_NAME     = process.env.DEMO_NAME     ?? 'Demo User';

export const DEMO_USER: AuthUser = { userId: 'user_demo', email: DEMO_EMAIL, name: DEMO_NAME };

// ─── Minimal self-signed token (base64url payload + HMAC-SHA256 sig) ──────────

function hmac(data: string): string {
    return crypto.createHmac('sha256', JWT_SECRET).update(data).digest('base64url');
}

export function issueToken(user: AuthUser, ttlSeconds = 7 * 24 * 3600): string {
    const payload = Buffer.from(
        JSON.stringify({ ...user, exp: Math.floor(Date.now() / 1000) + ttlSeconds })
    ).toString('base64url');
    return `${payload}.${hmac(payload)}`;
}

export function verifyToken(token: string): AuthUser | null {
    try {
        const dot = token.lastIndexOf('.');
        if (dot < 0) return null;
        const payload = token.slice(0, dot);
        const sig     = token.slice(dot + 1);
        if (hmac(payload) !== sig) return null;
        const data = JSON.parse(Buffer.from(payload, 'base64url').toString()) as AuthUser & { exp: number };
        if (data.exp < Math.floor(Date.now() / 1000)) return null;
        return { userId: data.userId, email: data.email, name: data.name };
    } catch {
        return null;
    }
}

export function extractUser(req: Request): AuthUser {
    if (!AUTH_ENABLED) return DEMO_USER;
    const header = req.headers.authorization;
    if (header?.startsWith('Bearer ')) {
        const user = verifyToken(header.slice(7));
        if (user) return user;
    }
    return DEMO_USER;
}

export function requireAuth(req: Request, res: Response, next: NextFunction): void {
    const user = extractUser(req);
    (req as AuthedRequest).user = user;
    next();
}

// Validate email + password against registered users.
// In demo/dev mode a single magic password is accepted for any email.
const MAGIC_PASSWORD = process.env.DEMO_PASSWORD ?? 'demo';

export function authenticatePassword(email: string, password: string): AuthUser | null {
    if (!AUTH_ENABLED) {
        if (password === MAGIC_PASSWORD) {
            return { userId: `user_${Buffer.from(email).toString('base64url').slice(0, 12)}`, email, name: email.split('@')[0] };
        }
        return null;
    }
    return null; // real auth delegates to Clerk/Auth0 — backend only verifies the JWT they issue
}
