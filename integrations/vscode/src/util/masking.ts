const MAX_MASK_LENGTH = 8;

export function validateRegexPattern(pattern: string): boolean {
    try {
        new RegExp(pattern);
        return true;
    } catch {
        return false;
    }
}

export function maskValue(name: string, value: string, patterns: string[]): { masked: string; shouldMask: boolean } {
    const shouldMask = patterns.some(pattern => {
        try {
            const regex = new RegExp(pattern);
            return regex.test(name);
        } catch (error) {
            // Log invalid regex patterns for debugging
            console.warn(`Invalid regex pattern skipped: ${pattern}`, error);
            return false;
        }
    });

    if (!shouldMask) {
        return { masked: value, shouldMask: false };
    }

    // Preserve length for better UX
    const masked = '•'.repeat(Math.min(value.length, MAX_MASK_LENGTH));
    return { masked, shouldMask: true };
}

export class MaskingService {
    private _revealed: Set<string> = new Set();

    shouldReveal(name: string): boolean {
        return this._revealed.has(name);
    }

    toggleReveal(name: string): void {
        if (this._revealed.has(name)) {
            this._revealed.delete(name);
        } else {
            this._revealed.add(name);
        }
    }

    clearRevealed(): void {
        this._revealed.clear();
    }

    maskVariable(name: string, value: string, patterns: string[]): { displayValue: string; isRevealed: boolean } {
        const { masked, shouldMask } = maskValue(name, value, patterns);
        
        if (!shouldMask) {
            return { displayValue: value, isRevealed: false };
        }

        const isRevealed = this.shouldReveal(name);
        return {
            displayValue: isRevealed ? value : masked,
            isRevealed
        };
    }
}