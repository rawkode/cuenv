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

export async function hashFileStream(filePath: string): Promise<string | null> {
    try {
        const hash = crypto.createHash('sha256');
        const stream = fs.createReadStream(filePath);
        
        for await (const chunk of stream) {
            hash.update(chunk);
        }
        
        return hash.digest('hex');
    } catch {
        return null;
    }
}

export function hashString(content: string): string {
    return crypto.createHash('sha256').update(content).digest('hex');
}