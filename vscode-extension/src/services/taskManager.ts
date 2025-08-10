import * as vscode from 'vscode';
import { EventEmitter, Event } from 'vscode';
import { CLIAdapter } from './cliAdapter';
import { Logger } from './logger';
import { TaskDefinition } from '../types/task';

export class TaskManager {
    private _onDidChangeTasks = new EventEmitter<TaskDefinition[]>();
    private _onTaskStarted = new EventEmitter<string>();
    private _onTaskFinished = new EventEmitter<{ name: string; success: boolean }>();
    private _onTaskError = new EventEmitter<{ name: string; error: string }>();
    
    private _tasks: TaskDefinition[] = [];
    private _terminals = new Map<string, vscode.Terminal>();
    private _disposables: vscode.Disposable[] = [];

    constructor(
        private workspaceFolder: vscode.WorkspaceFolder,
        private cliAdapter: CLIAdapter,
        private logger: Logger,
        private terminalStrategy: 'shared' | 'new'
    ) {
        // Clean up disposed terminals
        this._disposables.push(
            vscode.window.onDidCloseTerminal(terminal => {
                for (const [key, term] of this._terminals.entries()) {
                    if (term === terminal) {
                        this._terminals.delete(key);
                        break;
                    }
                }
            })
        );
    }

    get onDidChangeTasks(): Event<TaskDefinition[]> {
        return this._onDidChangeTasks.event;
    }

    get onTaskStarted(): Event<string> {
        return this._onTaskStarted.event;
    }

    get onTaskFinished(): Event<{ name: string; success: boolean }> {
        return this._onTaskFinished.event;
    }

    get onTaskError(): Event<{ name: string; error: string }> {
        return this._onTaskError.event;
    }

    get tasks(): TaskDefinition[] {
        return this._tasks;
    }

    async fetchTasks(): Promise<void> {
        try {
            const tasks = await this.cliAdapter.listTasks(this.workspaceFolder.uri.fsPath);
            this._tasks = tasks;
            this._onDidChangeTasks.fire(tasks);
            this.logger.debug(`Fetched ${tasks.length} tasks for ${this.workspaceFolder.name}`);
        } catch (error) {
            this.logger.error(`Failed to fetch tasks for ${this.workspaceFolder.name}`, error);
            this._onTaskError.fire({
                name: 'fetchTasks',
                error: error instanceof Error ? error.message : String(error)
            });
        }
    }

    getTaskByName(name: string): TaskDefinition | undefined {
        return this._tasks.find(task => task.name === name);
    }

    private getOrCreateTerminal(taskName?: string): vscode.Terminal {
        const terminalKey = this.terminalStrategy === 'shared' ? 'shared' : (taskName || 'task');
        
        let terminal = this._terminals.get(terminalKey);
        if (!terminal || terminal.exitStatus !== undefined) {
            // Terminal doesn't exist or has exited, create a new one
            const terminalName = this.terminalStrategy === 'shared' 
                ? `cuenv: ${this.workspaceFolder.name}`
                : `cuenv: ${taskName || 'task'}`;
                
            terminal = vscode.window.createTerminal({
                name: terminalName,
                cwd: this.workspaceFolder.uri.fsPath
            });
            
            this._terminals.set(terminalKey, terminal);
        }
        
        return terminal;
    }

    async runTask(taskName: string, useNewTerminal: boolean = false): Promise<void> {
        const task = this.getTaskByName(taskName);
        if (!task) {
            const error = `Task '${taskName}' not found`;
            this.logger.error(error);
            this._onTaskError.fire({ name: taskName, error });
            return;
        }

        try {
            this._onTaskStarted.fire(taskName);
            this.logger.info(`Starting task: ${taskName}`);

            if (useNewTerminal || this.terminalStrategy === 'new') {
                // Run in terminal for interactive output
                const terminal = this.getOrCreateTerminal(taskName);
                terminal.show();
                terminal.sendText(`cuenv internal task-protocol --run-task "${taskName}"`);
            } else {
                // Use shared terminal
                const terminal = this.getOrCreateTerminal();
                terminal.show();
                terminal.sendText(`cuenv internal task-protocol --run-task "${taskName}"`);
            }

            // Note: We can't easily detect terminal command completion from here
            // The terminal handles the task execution and shows output
            this._onTaskFinished.fire({ name: taskName, success: true });
            
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            this.logger.error(`Failed to run task '${taskName}'`, error);
            this._onTaskError.fire({ name: taskName, error: errorMessage });
        }
    }

    updateTerminalStrategy(strategy: 'shared' | 'new'): void {
        this.terminalStrategy = strategy;
    }

    dispose(): void {
        this._onDidChangeTasks.dispose();
        this._onTaskStarted.dispose();
        this._onTaskFinished.dispose();
        this._onTaskError.dispose();
        this._disposables.forEach(d => d.dispose());
        
        // Dispose terminals
        for (const terminal of this._terminals.values()) {
            terminal.dispose();
        }
        this._terminals.clear();
    }
}