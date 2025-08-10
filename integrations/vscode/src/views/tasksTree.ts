import * as vscode from 'vscode';
import { TaskManager } from '../services/taskManager';
import { TaskItem, TaskDefinition } from '../types/task';

export class TasksTreeDataProvider implements vscode.TreeDataProvider<TaskItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<TaskItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private disposables: vscode.Disposable[] = [];

    constructor(
        private taskManager: TaskManager,
        private workspaceFolder: vscode.WorkspaceFolder
    ) {
        this.disposables.push(
            this.taskManager.onDidChangeTasks(() => this.refresh())
        );
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: TaskItem): vscode.TreeItem {
        const item = new vscode.TreeItem(
            element.name,
            vscode.TreeItemCollapsibleState.None
        );
        
        item.contextValue = 'task';
        item.iconPath = new vscode.ThemeIcon('play');
        
        // Create description with dependencies if any
        let description = element.description || '';
        if (element.dependencies.length > 0) {
            const depText = `Dependencies: ${element.dependencies.join(', ')}`;
            description = description ? `${description} • ${depText}` : depText;
        }
        item.description = description;
        
        item.tooltip = this.createTooltip(element);
        
        // Add command to run task on click
        item.command = {
            command: 'cuenv.runTask',
            title: 'Run Task',
            arguments: [element.name, false] // false = use default terminal strategy
        };

        return item;
    }

    private createTooltip(element: TaskItem): string {
        const parts = [`Task: ${element.name}`];
        
        if (element.description) {
            parts.push(`Description: ${element.description}`);
        }
        
        if (element.dependencies.length > 0) {
            parts.push(`Dependencies: ${element.dependencies.join(', ')}`);
        }
        
        parts.push('Click to run task');
        
        return parts.join('\n');
    }

    getChildren(element?: TaskItem): Thenable<TaskItem[]> {
        if (element) {
            return Promise.resolve([]);
        }

        const tasks = this.taskManager.tasks;
        const taskItems: TaskItem[] = tasks.map(task => ({
            name: task.name,
            description: task.description,
            dependencies: task.after || []
        }));

        // Sort alphabetically by task name
        taskItems.sort((a, b) => a.name.localeCompare(b.name));
        
        return Promise.resolve(taskItems);
    }

    async runTask(taskName: string, useNewTerminal: boolean = false): Promise<void> {
        await this.taskManager.runTask(taskName, useNewTerminal);
    }

    async revealTaskDefinition(taskName: string): Promise<void> {
        const envCueUri = vscode.Uri.joinPath(this.workspaceFolder.uri, 'env.cue');
        
        try {
            const document = await vscode.workspace.openTextDocument(envCueUri);
            const editor = await vscode.window.showTextDocument(document);
            
            // Search for the task definition in the file
            const text = document.getText();
            const taskRegex = new RegExp(`"${taskName}"\\s*:\\s*\\{`, 'g');
            const match = taskRegex.exec(text);
            
            if (match) {
                const position = document.positionAt(match.index);
                const range = new vscode.Range(position, position);
                editor.selection = new vscode.Selection(range.start, range.end);
                editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
            } else {
                vscode.window.showWarningMessage(`Could not find task definition for '${taskName}' in env.cue`);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open env.cue: ${error}`);
        }
    }

    dispose(): void {
        this._onDidChangeTreeData.dispose();
        this.disposables.forEach(d => d.dispose());
    }
}