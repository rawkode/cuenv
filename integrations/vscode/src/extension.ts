import * as vscode from 'vscode';
import { CLIAdapter } from './services/cliAdapter';
import { EnvironmentManager } from './services/environmentManager';
import { TaskManager } from './services/taskManager';
import { StatusBarService } from './services/statusBarService';
import { ConfigurationService } from './services/configurationService';
import { Logger } from './services/logger';
import { EnvTreeDataProvider } from './views/envTree';
import { TasksTreeDataProvider } from './views/tasksTree';
import { TaskCodeLensProvider } from './lenses/taskCodeLens';

class CuenvExtension {
    private logger: Logger;
    private configService: ConfigurationService;
    private statusBarService: StatusBarService;
    private environmentManagers = new Map<string, EnvironmentManager>();
    private taskManagers = new Map<string, TaskManager>();
    private envTreeProviders = new Map<string, EnvTreeDataProvider>();
    private tasksTreeProviders = new Map<string, TasksTreeDataProvider>();
    private codeLensProviders = new Map<string, TaskCodeLensProvider>();
    private disposables: vscode.Disposable[] = [];

    constructor(private context: vscode.ExtensionContext) {
        this.logger = new Logger();
        this.configService = new ConfigurationService();
        this.statusBarService = new StatusBarService(this.logger);

        this.disposables.push(
            this.logger,
            this.configService,
            this.statusBarService
        );
    }

    async activate(): Promise<void> {
        this.logger.info('Activating cuenv extension');

        // Register commands
        this.registerCommands();

        // Setup workspace folder management
        this.setupWorkspaceFolders();

        // Initialize existing workspace folders
        if (vscode.workspace.workspaceFolders) {
            for (const folder of vscode.workspace.workspaceFolders) {
                await this.initializeWorkspaceFolder(folder);
            }
        }

        // Setup configuration change handling
        this.configService.onDidChangeConfiguration(config => {
            this.handleConfigurationChange(config);
        });

        this.logger.info('cuenv extension activated');
    }

    private registerCommands(): void {
        const commands = [
            vscode.commands.registerCommand('cuenv.reload', () => this.reloadCurrentEnvironment()),
            vscode.commands.registerCommand('cuenv.viewOutput', () => this.logger.show()),
            vscode.commands.registerCommand('cuenv.toggleAutoLoad', () => this.toggleAutoLoad()),
            vscode.commands.registerCommand('cuenv.runTask', (taskName: string, useNewTerminal: boolean = false) => 
                this.runTask(taskName, useNewTerminal)),
            vscode.commands.registerCommand('cuenv.refreshEnvPanel', () => this.refreshEnvPanel()),
            vscode.commands.registerCommand('cuenv.refreshTasksPanel', () => this.refreshTasksPanel()),
            vscode.commands.registerCommand('cuenv.toggleMasking', () => this.toggleMasking()),
            vscode.commands.registerCommand('cuenv.copyEnvName', (item) => this.copyEnvName(item)),
            vscode.commands.registerCommand('cuenv.copyEnvValue', (item) => this.copyEnvValue(item)),
            vscode.commands.registerCommand('cuenv.runTaskInNewTerminal', (item) => 
                this.runTask(item.name, true)),
            vscode.commands.registerCommand('cuenv.revealTaskDefinition', (item) => 
                this.revealTaskDefinition(item.name)),
            vscode.commands.registerCommand('cuenv.showQuickPick', () => this.statusBarService.showQuickPick())
        ];

        this.disposables.push(...commands);
    }

    private setupWorkspaceFolders(): void {
        this.disposables.push(
            vscode.workspace.onDidChangeWorkspaceFolders(e => {
                e.removed.forEach(folder => this.cleanupWorkspaceFolder(folder));
                e.added.forEach(folder => this.initializeWorkspaceFolder(folder));
            })
        );
    }

    private async initializeWorkspaceFolder(folder: vscode.WorkspaceFolder): Promise<void> {
        const key = folder.uri.fsPath;
        
        if (this.environmentManagers.has(key)) {
            // Already initialized
            return;
        }

        this.logger.info(`Initializing workspace folder: ${folder.name}`);

        const config = this.configService.getConfiguration();
        const cliAdapter = new CLIAdapter(config.executablePath);

        // Create managers
        const environmentManager = new EnvironmentManager(
            folder,
            cliAdapter,
            this.logger,
            config.watchDebounceMs
        );

        const taskManager = new TaskManager(
            folder,
            cliAdapter,
            this.logger,
            config.terminalStrategy
        );

        // Store managers
        this.environmentManagers.set(key, environmentManager);
        this.taskManagers.set(key, taskManager);

        // Add to status bar
        this.statusBarService.addEnvironmentManager(folder, environmentManager);

        // Create tree data providers
        const envTreeProvider = new EnvTreeDataProvider(environmentManager, this.configService);
        const tasksTreeProvider = new TasksTreeDataProvider(taskManager, folder);
        const codeLensProvider = new TaskCodeLensProvider(taskManager, this.logger);

        this.envTreeProviders.set(key, envTreeProvider);
        this.tasksTreeProviders.set(key, tasksTreeProvider);
        this.codeLensProviders.set(key, codeLensProvider);

        // Register tree data providers (only register once globally)
        if (this.envTreeProviders.size === 1) {
            this.disposables.push(
                vscode.window.registerTreeDataProvider('cuenv-env', this.getCurrentEnvTreeProvider()),
                vscode.window.registerTreeDataProvider('cuenv-tasks', this.getCurrentTasksTreeProvider()),
                vscode.languages.registerCodeLensProvider(
                    { scheme: 'file', pattern: '**/env.cue' },
                    this.getCurrentCodeLensProvider()
                )
            );
        }

        // Load environment if auto-load is enabled
        if (config.autoLoadEnabled) {
            await environmentManager.load();
            await taskManager.fetchTasks();
        }

        this.logger.info(`Workspace folder initialized: ${folder.name}`);
    }

    private cleanupWorkspaceFolder(folder: vscode.WorkspaceFolder): void {
        const key = folder.uri.fsPath;
        
        this.logger.info(`Cleaning up workspace folder: ${folder.name}`);

        // Dispose managers
        this.environmentManagers.get(key)?.dispose();
        this.taskManagers.get(key)?.dispose();
        this.envTreeProviders.get(key)?.dispose();
        this.tasksTreeProviders.get(key)?.dispose();
        this.codeLensProviders.get(key)?.dispose();

        // Remove from maps
        this.environmentManagers.delete(key);
        this.taskManagers.delete(key);
        this.envTreeProviders.delete(key);
        this.tasksTreeProviders.delete(key);
        this.codeLensProviders.delete(key);
    }

    // Helper methods to get current providers based on active workspace folder
    private getCurrentEnvTreeProvider(): EnvTreeDataProvider {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const provider = this.envTreeProviders.get(folder.uri.fsPath);
            if (provider) return provider;
        }
        // Return first available provider
        return Array.from(this.envTreeProviders.values())[0];
    }

    private getCurrentTasksTreeProvider(): TasksTreeDataProvider {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const provider = this.tasksTreeProviders.get(folder.uri.fsPath);
            if (provider) return provider;
        }
        // Return first available provider
        return Array.from(this.tasksTreeProviders.values())[0];
    }

    private getCurrentCodeLensProvider(): TaskCodeLensProvider {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const provider = this.codeLensProviders.get(folder.uri.fsPath);
            if (provider) return provider;
        }
        // Return first available provider
        return Array.from(this.codeLensProviders.values())[0];
    }

    private getCurrentWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor) {
            return vscode.workspace.getWorkspaceFolder(activeEditor.document.uri);
        }
        return vscode.workspace.workspaceFolders?.[0];
    }

    private handleConfigurationChange(config: any): void {
        // Update CLI adapter executable paths
        for (const manager of this.environmentManagers.values()) {
            manager.updateExecutablePath(config.executablePath);
        }

        // Update debounce settings
        for (const manager of this.environmentManagers.values()) {
            manager.updateDebounceMs(config.watchDebounceMs);
        }

        // Update terminal strategy
        for (const manager of this.taskManagers.values()) {
            manager.updateTerminalStrategy(config.terminalStrategy);
        }
    }

    // Command implementations
    private async reloadCurrentEnvironment(): Promise<void> {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const manager = this.environmentManagers.get(folder.uri.fsPath);
            await manager?.reload();
        }
    }

    private async toggleAutoLoad(): Promise<void> {
        const config = vscode.workspace.getConfiguration('cuenv');
        const currentValue = config.get('autoLoad.enabled', true);
        await config.update('autoLoad.enabled', !currentValue, vscode.ConfigurationTarget.Global);
        
        const newState = !currentValue ? 'enabled' : 'disabled';
        vscode.window.showInformationMessage(`Auto-load ${newState}`);
    }

    private async runTask(taskName: string, useNewTerminal: boolean): Promise<void> {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const manager = this.taskManagers.get(folder.uri.fsPath);
            await manager?.runTask(taskName, useNewTerminal);
        }
    }

    private refreshEnvPanel(): void {
        const provider = this.getCurrentEnvTreeProvider();
        provider?.refresh();
    }

    private refreshTasksPanel(): void {
        const provider = this.getCurrentTasksTreeProvider();
        provider?.refresh();
    }

    private toggleMasking(): void {
        const provider = this.getCurrentEnvTreeProvider();
        provider?.toggleMasking();
    }

    private async copyEnvName(item: any): Promise<void> {
        const provider = this.getCurrentEnvTreeProvider();
        await provider?.copyEnvName(item);
    }

    private async copyEnvValue(item: any): Promise<void> {
        const provider = this.getCurrentEnvTreeProvider();
        await provider?.copyEnvValue(item);
    }

    private async revealTaskDefinition(taskName: string): Promise<void> {
        const folder = this.getCurrentWorkspaceFolder();
        if (folder) {
            const provider = this.tasksTreeProviders.get(folder.uri.fsPath);
            await provider?.revealTaskDefinition(taskName);
        }
    }

    dispose(): void {
        this.disposables.forEach(d => d.dispose());
        
        // Clean up all workspace folders
        for (const folder of Array.from(this.environmentManagers.keys())) {
            const workspaceFolder = vscode.workspace.workspaceFolders?.find(f => f.uri.fsPath === folder);
            if (workspaceFolder) {
                this.cleanupWorkspaceFolder(workspaceFolder);
            }
        }
    }
}

let extensionInstance: CuenvExtension | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    extensionInstance = new CuenvExtension(context);
    await extensionInstance.activate();
}

export function deactivate(): void {
    extensionInstance?.dispose();
    extensionInstance = undefined;
}