import * as assert from 'assert';
import * as vscode from 'vscode';
import { TaskCodeLensProvider } from '../src/lenses/taskCodeLens';
import { TaskManager } from '../src/services/taskManager';
import { Logger } from '../src/services/logger';
import { TaskDefinition } from '../src/types/task';

// Mock VSCode APIs
const mockEventEmitter = {
    fire: () => {},
    event: () => {},
    dispose: () => {}
};

class MockTaskManager {
    private _onDidChangeTasks = mockEventEmitter;
    private _tasks: TaskDefinition[] = [];

    get onDidChangeTasks() {
        return this._onDidChangeTasks.event;
    }

    get tasks(): TaskDefinition[] {
        return this._tasks;
    }

    setTasks(tasks: TaskDefinition[]) {
        this._tasks = tasks;
    }
}

class MockLogger {
    debug() {}
    info() {}
    error() {}
}

class MockDocument implements vscode.TextDocument {
    uri!: vscode.Uri;
    fileName!: string;
    isUntitled!: boolean;
    languageId!: string;
    version!: number;
    isDirty!: boolean;
    isClosed!: boolean;
    eol!: vscode.EndOfLine;
    lineCount!: number;
    
    private _text: string = '';

    constructor(fileName: string, text: string) {
        this.fileName = fileName;
        this._text = text;
    }

    getText(): string {
        return this._text;
    }

    positionAt(offset: number): vscode.Position {
        const lines = this._text.substring(0, offset).split('\n');
        const line = lines.length - 1;
        const character = lines[lines.length - 1].length;
        return new vscode.Position(line, character);
    }

    save(): Thenable<boolean> { throw new Error('Not implemented'); }
    lineAt(): vscode.TextLine { throw new Error('Not implemented'); }
    offsetAt(): number { throw new Error('Not implemented'); }
    getWordRangeAtPosition(): vscode.Range | undefined { throw new Error('Not implemented'); }
    validateRange(): vscode.Range { throw new Error('Not implemented'); }
    validatePosition(): vscode.Position { throw new Error('Not implemented'); }
}

suite('TaskCodeLensProvider Tests', () => {
    let mockTaskManager: MockTaskManager;
    let mockLogger: MockLogger;
    let provider: TaskCodeLensProvider;

    setup(() => {
        mockTaskManager = new MockTaskManager();
        mockLogger = new MockLogger();
        provider = new TaskCodeLensProvider(mockTaskManager as any, mockLogger as any);
    });

    teardown(() => {
        provider.dispose();
    });

    suite('provideCodeLenses', () => {
        test('should return empty array for non-env.cue files', async () => {
            const doc = new MockDocument('test.ts', 'content');
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 0);
        });

        test('should return empty array when no tasks are available', async () => {
            const doc = new MockDocument('env.cue', 'some content');
            mockTaskManager.setTasks([]);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 0);
        });

        test('should find quoted task names in document', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'build', description: 'Build project', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `tasks: {
    "build": {
        description: "Build the project"
        command: ["bun", "run", "build"]
    }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 1);
            assert.strictEqual(result[0].command?.title, '▶ Run Task build');
            assert.strictEqual(result[0].command?.command, 'cuenv.runTask');
            assert.deepStrictEqual(result[0].command?.arguments, ['build', false]);
        });

        test('should find unquoted task names in document', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'test', description: 'Run tests', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `tasks: {
    test: {
        description: "Run all tests"
        command: ["bun", "test"]
    }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 1);
            assert.strictEqual(result[0].command?.title, '▶ Run Task test');
        });

        test('should handle task names with special regex characters', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'build-prod', description: 'Build for production', dependencies: [] },
                { name: 'test.unit', description: 'Unit tests', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `tasks: {
    "build-prod": {
        description: "Production build"
        command: ["bun", "run", "build:prod"]
    }
    "test.unit": {
        description: "Unit tests"
        command: ["bun", "run", "test:unit"]
    }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 2);
            
            const titles = result.map(cl => cl.command?.title).sort();
            assert.deepStrictEqual(titles, ['▶ Run Task build-prod', '▶ Run Task test.unit']);
        });

        test('should handle multiple occurrences of same task name', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'build', description: 'Build project', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `environments: {
    dev: {
        tasks: {
            build: { command: ["bun", "run", "build:dev"] }
        }
    }
    prod: {
        tasks: {
            build: { command: ["bun", "run", "build:prod"] }
        }
    }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 2);
            assert.ok(result.every(cl => cl.command?.title === '▶ Run Task build'));
        });

        test('should only match task definitions with colon and brace', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'build', description: 'Build project', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `
// This should not match
description: "This mentions build but should not match"
build_command: "bun run build"

// This should match
tasks: {
    build: {
        command: ["make", "build"]
    }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 1);
        });

        test('should handle tasks with complex names containing regex metacharacters', async () => {
            const tasks: TaskDefinition[] = [
                { name: 'test[unit]', description: 'Unit tests', dependencies: [] },
                { name: 'build.*', description: 'Build all', dependencies: [] },
                { name: 'deploy+prod', description: 'Deploy to prod', dependencies: [] }
            ];
            mockTaskManager.setTasks(tasks);

            const content = `tasks: {
    "test[unit]": { command: ["test"] }
    "build.*": { command: ["build"] }
    "deploy+prod": { command: ["deploy"] }
}`;
            const doc = new MockDocument('env.cue', content);
            
            const result = await provider.provideCodeLenses(doc, {} as any);
            assert.strictEqual(result.length, 3);
            
            const taskNames = result.map(cl => cl.command?.arguments?.[0]).sort();
            assert.deepStrictEqual(taskNames, ['build.*', 'deploy+prod', 'test[unit]']);
        });
    });

    suite('resolveCodeLens', () => {
        test('should return the same codeLens unchanged', () => {
            const range = new vscode.Range(0, 0, 0, 0);
            const command = { title: 'Test', command: 'test' };
            const codeLens = new vscode.CodeLens(range, command);
            
            const result = provider.resolveCodeLens(codeLens, {} as any);
            assert.strictEqual(result, codeLens);
        });
    });

    suite('dispose', () => {
        test('should dispose resources without errors', () => {
            assert.doesNotThrow(() => {
                provider.dispose();
            });
        });
    });
});