import * as vscode from 'vscode';
import { EnvironmentManager } from '../services/environmentManager';
import { ConfigurationService } from '../services/configurationService';
import { MaskingService } from '../util/masking';
import { EnvItem } from '../types/environment';

export class EnvTreeDataProvider implements vscode.TreeDataProvider<EnvItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<EnvItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private maskingService = new MaskingService();
    private disposables: vscode.Disposable[] = [];

    constructor(
        private environmentManager: EnvironmentManager,
        private configService: ConfigurationService
    ) {
        this.disposables.push(
            this.environmentManager.onDidChangeEnvironment(() => this.refresh()),
            this.configService.onDidChangeConfiguration(() => this.refresh())
        );
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: EnvItem): vscode.TreeItem {
        const item = new vscode.TreeItem(
            `${element.name}=${element.value}`,
            vscode.TreeItemCollapsibleState.None
        );
        
        item.contextValue = 'envVar';
        item.tooltip = `${element.name}: ${element.originalValue || element.value}`;
        
        // Add icon based on whether it's masked
        if (element.masked) {
            item.iconPath = new vscode.ThemeIcon('eye-closed');
        } else {
            item.iconPath = new vscode.ThemeIcon('symbol-variable');
        }

        return item;
    }

    getChildren(element?: EnvItem): Thenable<EnvItem[]> {
        if (element) {
            return Promise.resolve([]);
        }

        const environment = this.environmentManager.environment;
        if (!environment) {
            return Promise.resolve([]);
        }

        const config = this.configService.getConfiguration();
        const envItems: EnvItem[] = [];

        for (const [name, value] of Object.entries(environment.variables)) {
            const { displayValue, isRevealed } = this.maskingService.maskVariable(
                name, 
                value, 
                config.maskPatterns
            );
            
            envItems.push({
                name,
                value: displayValue,
                masked: displayValue !== value,
                originalValue: displayValue !== value ? value : undefined
            });
        }

        // Sort alphabetically by variable name
        envItems.sort((a, b) => a.name.localeCompare(b.name));
        
        return Promise.resolve(envItems);
    }

    async copyEnvName(item: EnvItem): Promise<void> {
        await vscode.env.clipboard.writeText(item.name);
        vscode.window.showInformationMessage(`Copied variable name: ${item.name}`);
    }

    async copyEnvValue(item: EnvItem): Promise<void> {
        // Always copy the original (unmasked) value
        const valueToCopy = item.originalValue || item.value;
        await vscode.env.clipboard.writeText(valueToCopy);
        vscode.window.showInformationMessage(`Copied value for: ${item.name}`);
    }

    toggleMasking(): void {
        this.maskingService.clearRevealed();
        this.refresh();
    }

    toggleRevealVariable(item: EnvItem): void {
        this.maskingService.toggleReveal(item.name);
        this.refresh();
    }

    dispose(): void {
        this._onDidChangeTreeData.dispose();
        this.disposables.forEach(d => d.dispose());
    }
}