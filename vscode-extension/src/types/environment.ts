import { Event } from 'vscode';

export interface EnvironmentJson {
    variables: Record<string, string>;
}

export enum EnvironmentState {
    Loaded = 'loaded',
    Error = 'error',
    PendingReload = 'pending-reload',
    NotFound = 'not-found',
    BinaryNotFound = 'binary-not-found',
    Disabled = 'disabled'
}

export interface EnvironmentEvents {
    onDidChangeEnvironment: Event<EnvironmentState>;
}

export interface EnvItem {
    name: string;
    value: string;
    masked: boolean;
    originalValue?: string;
}