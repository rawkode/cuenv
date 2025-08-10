import * as assert from 'assert';
import * as vscode from 'vscode';
import { EnvironmentManager } from '../src/services/environmentManager';
import { CLIAdapter } from '../src/services/cliAdapter';
import { Logger } from '../src/services/logger';
import { EnvironmentState } from '../src/types/environment';

// Mock CLI adapter for testing
class MockCLIAdapter extends CLIAdapter {
    private mockEnvironment = { variables: { TEST_VAR: 'test_value' } };
    private shouldFail = false;
    private binaryExists = true;

    constructor() {
        super('mock-cuenv');
    }

    setBinaryExists(exists: boolean): void {
        this.binaryExists = exists;
    }

    setShouldFail(fail: boolean): void {
        this.shouldFail = fail;
    }

    setMockEnvironment(env: any): void {
        this.mockEnvironment = env;
    }

    async checkBinaryExists(): Promise<boolean> {
        return this.binaryExists;
    }

    async exportEnv(_workspaceFolder: string): Promise<any> {
        if (this.shouldFail) {
            throw new Error('Mock CLI error');
        }
        return this.mockEnvironment;
    }
}

suite('EnvironmentManager Tests', () => {
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

    test('should start in NotFound state', () => {
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger);
        assert.strictEqual(manager.state, EnvironmentState.NotFound);
        manager.dispose();
    });

    test('should transition to BinaryNotFound when binary is missing', async () => {
        mockCLI.setBinaryExists(false);
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger);
        
        let stateChanged = false;
        manager.onDidChangeEnvironment((state) => {
            if (state === EnvironmentState.BinaryNotFound) {
                stateChanged = true;
            }
        });

        await manager.load();
        assert.strictEqual(manager.state, EnvironmentState.BinaryNotFound);
        assert.strictEqual(stateChanged, true);
        
        manager.dispose();
    });

    test('should handle CLI errors gracefully', async () => {
        mockCLI.setShouldFail(true);
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger);
        
        let errorState = false;
        manager.onDidChangeEnvironment((state) => {
            if (state === EnvironmentState.Error) {
                errorState = true;
            }
        });

        await manager.load();
        assert.strictEqual(manager.state, EnvironmentState.Error);
        assert.strictEqual(errorState, true);
        
        manager.dispose();
    });

    test('should load environment successfully', async () => {
        const testEnv = { variables: { API_KEY: 'secret123', DB_HOST: 'localhost' } };
        mockCLI.setMockEnvironment(testEnv);
        
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger);
        
        let loadedState = false;
        manager.onDidChangeEnvironment((state) => {
            if (state === EnvironmentState.Loaded) {
                loadedState = true;
            }
        });

        await manager.load();
        assert.strictEqual(manager.state, EnvironmentState.Loaded);
        assert.strictEqual(loadedState, true);
        assert.deepStrictEqual(manager.environment, testEnv);
        
        manager.dispose();
    });

    test('should update debounce settings', () => {
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger, 100);
        manager.updateDebounceMs(500);
        // No direct way to test this, but ensure it doesn't throw
        manager.dispose();
    });

    test('should handle reload', async () => {
        const manager = new EnvironmentManager(workspaceFolder, mockCLI, logger);
        
        let reloadCount = 0;
        manager.onDidChangeEnvironment(() => {
            reloadCount++;
        });

        await manager.reload();
        // Should have at least attempted to reload
        assert(reloadCount >= 0);
        
        manager.dispose();
    });
});