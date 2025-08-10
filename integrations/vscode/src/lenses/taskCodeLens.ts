import * as vscode from 'vscode';
import { TaskManager } from '../services/taskManager';
import { Logger } from '../services/logger';

export class TaskCodeLensProvider implements vscode.CodeLensProvider {
    private _onDidChangeCodeLenses = new vscode.EventEmitter<void>();
    private disposables: vscode.Disposable[] = [];

    public readonly onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;

    constructor(
        private taskManager: TaskManager,
        private logger: Logger
    ) {
        this.disposables.push(
            this.taskManager.onDidChangeTasks(() => this._onDidChangeCodeLenses.fire()),
            vscode.workspace.onDidChangeTextDocument((e) => {
                if (e.document.fileName.endsWith('env.cue')) {
                    this._onDidChangeCodeLenses.fire();
                }
            })
        );
    }

    public async provideCodeLenses(
        document: vscode.TextDocument,
        _token: vscode.CancellationToken
    ): Promise<vscode.CodeLens[]> {
        if (!document.fileName.endsWith('env.cue')) {
            return [];
        }

        const codeLenses: vscode.CodeLens[] = [];
        const text = document.getText();
        const tasks = this.taskManager.tasks;

        // Find task definitions in the document
        for (const task of tasks) {
            const taskPattern = new RegExp(
                `("${task.name}"|${task.name})\\s*:\\s*\\{`,
                'g'
            );
            
            let match;
            while ((match = taskPattern.exec(text)) !== null) {
                const position = document.positionAt(match.index);
                const range = new vscode.Range(position, position);
                
                const codeLens = new vscode.CodeLens(range, {
                    title: `▶ Run Task ${task.name}`,
                    command: 'cuenv.runTask',
                    arguments: [task.name, false]
                });
                
                codeLenses.push(codeLens);
                this.logger.debug(`Added CodeLens for task: ${task.name} at line ${position.line + 1}`);
            }
        }

        return codeLenses;
    }

    public resolveCodeLens(
        codeLens: vscode.CodeLens,
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.CodeLens> {
        return codeLens;
    }

    dispose(): void {
        this._onDidChangeCodeLenses.dispose();
        this.disposables.forEach(d => d.dispose());
    }
}