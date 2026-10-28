#!/usr/bin/env python3
"""Test the release gate's failure paths with fake macOS command-line tools."""
import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('smoke-macos.sh').resolve()


class MacSmokeTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.app = self.root / 'source/Tractr.app/Contents/MacOS'
        self.app.mkdir(parents=True)
        self.write_executable(self.app / 'Tractr', 'exec /bin/sleep 60')
        (self.app / 'libfarm_ffi.dylib').write_bytes(b'native fixture')
        self.write_executable(self.bin / 'ditto', 'cp -R "$SOURCE/Tractr.app" "$3/"')
        self.write_executable(self.bin / 'sleep', '/bin/sleep 0.03')
        for tool in ['codesign', 'xcrun', 'spctl']:
            self.write_executable(self.bin / tool,
                                  f'echo {tool} >> "$CALLS"\n[ "${{FAIL_TOOL:-}}" != {tool} ]')
        self.env = {**os.environ, 'PATH': f'{self.bin}:{os.environ["PATH"]}',
                    'SOURCE': str(self.root / 'source'), 'CALLS': str(self.root / 'calls')}
        for target, executable in [('windows-x64', 'farm-player.exe'), ('linux-x64', 'farm-player'), ('web', 'farm_wasm_bg.wasm')]:
            folder = self.app / 'players' / target
            folder.mkdir(parents=True)
            names = [executable, 'THIRD-PARTY.txt']
            if target == 'web':
                names += ['farm_wasm.js', 'index.html', 'style.css', 'game.js']
            hashes = {}
            for name in names:
                (folder / name).write_bytes(b'fixture')
                hashes[name] = hashlib.sha256(b'fixture').hexdigest()
            (folder / 'template.json').write_text(json.dumps({
                'target': target, 'version': '1.0.0', 'sha256': hashes.pop(executable), 'files': hashes}))

    def write_executable(self, path, body):
        path.write_text('#!/usr/bin/env bash\nset -e\n' + body + '\n')
        path.chmod(0o755)

    def run_gate(self, signed=False):
        args = ['bash', str(SCRIPT), 'fixture.zip', '1.0.0']
        if signed:
            args.append('--require-signature')
        return subprocess.run(args, env=self.env, capture_output=True, text=True, timeout=10)

    def test_unsigned_dry_run_launches_without_signing_tools(self):
        result = self.run_gate()
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertFalse((self.root / 'calls').exists())

    def test_live_release_requires_all_signature_checks(self):
        result = self.run_gate(signed=True)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual(['codesign', 'xcrun', 'spctl'], (self.root / 'calls').read_text().splitlines())

    def test_each_signature_failure_blocks_release(self):
        for tool in ['codesign', 'xcrun', 'spctl']:
            with self.subTest(tool=tool):
                self.env['FAIL_TOOL'] = tool
                self.assertNotEqual(0, self.run_gate(signed=True).returncode)

    def test_missing_packaged_template_blocks_release(self):
        (self.app / 'players/web/farm_wasm.js').unlink()
        self.assertNotEqual(0, self.run_gate().returncode)

    def test_native_library_is_required(self):
        (self.app / 'libfarm_ffi.dylib').unlink()
        self.assertNotEqual(0, self.run_gate().returncode)

    def test_early_exit_blocks_release(self):
        self.write_executable(self.app / 'Tractr', 'exit 0')
        result = self.run_gate()
        self.assertNotEqual(0, result.returncode)
        self.assertIn('exited during startup', result.stderr)


if __name__ == '__main__':
    unittest.main()
