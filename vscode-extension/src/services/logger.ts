import * as vscode from 'vscode';
import { maskValue } from '../util/masking';

export class Logger {
    private outputChannel: vscode.OutputChannel;

    constructor() {
        this.outputChannel = vscode.window.createOutputChannel('cuenv');
    }

    private formatMessage(level: string, message: string): string {
        const timestamp = new Date().toISOString();
        return `[${timestamp}] ${level.toUpperCase()}: ${message}`;
    }

    private log(level: string, message: string): void {
        const formatted = this.formatMessage(level, message);
        this.outputChannel.appendLine(formatted);
    }

    info(message: string): void {
        this.log('info', message);
    }

    warn(message: string): void {
        this.log('warn', message);
    }

    error(message: string, error?: any): void {
        let errorMessage = message;
        if (error) {
            if (error instanceof Error) {
                errorMessage += `: ${error.message}`;
                if (error.stack) {
                    errorMessage += `\nStack: ${error.stack}`;
                }
            } else {
                errorMessage += `: ${String(error)}`;
            }
        }
        this.log('error', errorMessage);
    }

    debug(message: string): void {
        this.log('debug', message);
    }

    show(): void {
        this.outputChannel.show();
    }

    dispose(): void {
        this.outputChannel.dispose();
    }

    // Safe logging that masks sensitive values
    logEnvironment(variables: Record<string, string>, maskPatterns: string[]): void {
        this.info('Environment variables loaded:');
        Object.entries(variables).forEach(([name, value]) => {
            const { masked } = maskValue(name, value, maskPatterns);
            this.info(`  ${name}=${masked}`);
        });
    }
}