import * as vscode from 'vscode';
import { EventEmitter, Event } from 'vscode';

export interface CuenvConfiguration {
    executablePath: string;
    autoLoadEnabled: boolean;
    maskPatterns: string[];
    terminalStrategy: 'shared' | 'new';
    watchDebounceMs: number;
}

export class ConfigurationService {
    private _onDidChangeConfiguration = new EventEmitter<CuenvConfiguration>();
    private _disposables: vscode.Disposable[] = [];

    constructor() {
        this._disposables.push(
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('cuenv')) {
                    this._onDidChangeConfiguration.fire(this.getConfiguration());
                }
            })
        );
    }

    get onDidChangeConfiguration(): Event<CuenvConfiguration> {
        return this._onDidChangeConfiguration.event;
    }

    getConfiguration(): CuenvConfiguration {
        const config = vscode.workspace.getConfiguration('cuenv');
        return {
            executablePath: config.get('executablePath', 'cuenv'),
            autoLoadEnabled: config.get('autoLoad.enabled', true),
            maskPatterns: config.get('env.maskPatterns', ['(?i)(secret|token|password|key|api_key)']),
            terminalStrategy: config.get('tasks.terminal.strategy', 'shared') as 'shared' | 'new',
            watchDebounceMs: config.get('watch.debounceMs', 300)
        };
    }

    dispose(): void {
        this._onDidChangeConfiguration.dispose();
        this._disposables.forEach(d => d.dispose());
    }
}