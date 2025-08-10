import * as crypto from 'crypto';
import * as fs from 'fs';
import { promisify } from 'util';

const readFile = promisify(fs.readFile);

export async function hashFile(filePath: string): Promise<string | null> {
    try {
        const content = await readFile(filePath, 'utf8');
        return crypto.createHash('sha256').update(content).digest('hex');
    } catch {
        return null;
    }
}

export function hashString(content: string): string {
    return crypto.createHash('sha256').update(content).digest('hex');
}