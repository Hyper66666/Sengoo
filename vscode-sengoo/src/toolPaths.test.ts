import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { resolveBundledToolPath } from './toolPaths';

function makeTempDir(): string {
    return fs.mkdtempSync(path.join(os.tmpdir(), 'sengoo-vscode-toolpaths-'));
}

function touchFile(filePath: string, contents = ''): void {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, contents);
}

function cleanup(dir: string): void {
    fs.rmSync(dir, { recursive: true, force: true });
}

function testResolvesDirectDebugBinary(): void {
    const root = makeTempDir();
    try {
        const exe = process.platform === 'win32' ? 'sglsp.exe' : 'sglsp';
        const expected = path.join(root, 'target', 'debug', exe);
        touchFile(expected);

        const actual = resolveBundledToolPath([root], 'sglsp');
        assert.strictEqual(actual, expected);
    } finally {
        cleanup(root);
    }
}

function testFallsBackToDepsBinary(): void {
    const root = makeTempDir();
    try {
        const suffix = process.platform === 'win32' ? '.exe' : '';
        const older = path.join(root, 'target', 'debug', 'deps', `sglsp-older${suffix}`);
        const newer = path.join(root, 'target', 'debug', 'deps', `sglsp-newer${suffix}`);
        touchFile(older);
        touchFile(newer);
        const now = new Date();
        fs.utimesSync(older, now, new Date(now.getTime() - 10_000));
        fs.utimesSync(newer, now, new Date(now.getTime() + 10_000));

        const actual = resolveBundledToolPath([root], 'sglsp');
        assert.strictEqual(actual, newer);
    } finally {
        cleanup(root);
    }
}

testResolvesDirectDebugBinary();
testFallsBackToDepsBinary();
console.log('toolPaths tests passed');
