import * as assert from 'assert';
import * as vscode from 'vscode';
import { TaskManager } from '../src/services/taskManager';
import { CLIAdapter } from '../src/services/cliAdapter';
import { Logger } from '../src/services/logger';
import { TaskDefinition } from '../src/types/task';

// Mock CLI adapter for testing
class MockCLIAdapter extends CLIAdapter {
    private mockTasks: TaskDefinition[] = [
        { name: 'build', description: 'Build the application', after: [] },
        { name: 'test', description: 'Run tests', after: ['build'] },
        { name: 'deploy', description: 'Deploy application', after: ['test'] }
    ];
    private shouldFail = false;

    constructor() {
        super('mock-cuenv');
    }

    setShouldFail(fail: boolean): void {
        this.shouldFail = fail;
    }

    setMockTasks(tasks: TaskDefinition[]): void {
        this.mockTasks = tasks;
    }

    async listTasks(_workspaceFolder: string): Promise<TaskDefinition[]> {
        if (this.shouldFail) {
            throw new Error('Mock CLI error');
        }
        return this.mockTasks;
    }

    async runTask(_workspaceFolder: string, taskName: string): Promise<void> {
        if (this.shouldFail) {
            throw new Error(`Failed to run task: ${taskName}`);
        }
        // Mock successful task execution
    }
}

suite('TaskManager Tests', () => {
    let mockCLI: MockCLIAdapter;
    let logger: Logger;
    let workspaceFolder: vscode.WorkspaceFolder;

    setup(() => {
        mockCLI = new MockCLIAdapter();
        logger = new Logger();
        workspaceFolder = {
            uri: vscode.Uri.file('/test/workspace'),
            name: 'test-workspace',
            index: 0
        };
    });

    teardown(() => {
        logger.dispose();
    });

    test('should start with empty tasks', () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        assert.strictEqual(manager.tasks.length, 0);
        manager.dispose();
    });

    test('should fetch tasks successfully', async () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        
        let tasksChanged = false;
        let receivedTasks: TaskDefinition[] = [];
        
        manager.onDidChangeTasks((tasks) => {
            tasksChanged = true;
            receivedTasks = tasks;
        });

        await manager.fetchTasks();
        
        assert.strictEqual(tasksChanged, true);
        assert.strictEqual(manager.tasks.length, 3);
        assert.strictEqual(receivedTasks.length, 3);
        
        // Check task details
        const buildTask = manager.getTaskByName('build');
        assert.strictEqual(buildTask?.name, 'build');
        assert.strictEqual(buildTask?.description, 'Build the application');
        assert.deepStrictEqual(buildTask?.after, []);
        
        const testTask = manager.getTaskByName('test');
        assert.strictEqual(testTask?.name, 'test');
        assert.deepStrictEqual(testTask?.after, ['build']);
        
        manager.dispose();
    });

    test('should handle fetch errors gracefully', async () => {
        mockCLI.setShouldFail(true);
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        
        let errorReceived = false;
        manager.onTaskError((error) => {
            errorReceived = true;
            assert.strictEqual(error.name, 'fetchTasks');
        });

        await manager.fetchTasks();
        
        assert.strictEqual(errorReceived, true);
        assert.strictEqual(manager.tasks.length, 0);
        
        manager.dispose();
    });

    test('should find task by name', async () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        await manager.fetchTasks();
        
        const task = manager.getTaskByName('build');
        assert.strictEqual(task?.name, 'build');
        
        const nonExistentTask = manager.getTaskByName('nonexistent');
        assert.strictEqual(nonExistentTask, undefined);
        
        manager.dispose();
    });

    test('should handle task execution', async () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        await manager.fetchTasks();
        
        let taskStarted = false;
        let taskFinished = false;
        
        manager.onTaskStarted((taskName) => {
            taskStarted = true;
            assert.strictEqual(taskName, 'build');
        });
        
        manager.onTaskFinished((result) => {
            taskFinished = true;
            assert.strictEqual(result.name, 'build');
            assert.strictEqual(result.success, true);
        });

        await manager.runTask('build');
        
        assert.strictEqual(taskStarted, true);
        assert.strictEqual(taskFinished, true);
        
        manager.dispose();
    });

    test('should handle non-existent task execution', async () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        await manager.fetchTasks();
        
        let errorReceived = false;
        manager.onTaskError((error) => {
            errorReceived = true;
            assert.strictEqual(error.name, 'nonexistent');
            assert(error.error.includes('not found'));
        });

        await manager.runTask('nonexistent');
        assert.strictEqual(errorReceived, true);
        
        manager.dispose();
    });

    test('should update terminal strategy', () => {
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        
        // Should not throw
        manager.updateTerminalStrategy('new');
        
        manager.dispose();
    });

    test('should handle empty task list', async () => {
        mockCLI.setMockTasks([]);
        const manager = new TaskManager(workspaceFolder, mockCLI, logger, 'shared');
        
        await manager.fetchTasks();
        assert.strictEqual(manager.tasks.length, 0);
        
        manager.dispose();
    });
});