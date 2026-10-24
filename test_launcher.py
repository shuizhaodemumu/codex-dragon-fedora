#!/usr/bin/env python3
"""Integration tests with fake player/app; never launch the real Codex."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

BINARY = str(Path(__file__).resolve().parent / 'codex-dragon')

class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.video = self.root / 'video.mp4'
        self.video.write_bytes(b'test fixture, not a real video')
        self.player = self.root / 'ffplay'
        self.player.write_text('#!/bin/sh\nexit 0\n')
        self.player.chmod(0o755)
        self.app = self.root / 'app'
        self.app.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$TEST_APP_LOG"\n')
        self.app.chmod(0o755)
        self.log = self.root / 'app.log'
        self.env = dict(os.environ, PATH=str(self.root) + ':/usr/bin:/bin', XDG_RUNTIME_DIR=str(self.root), TEST_APP_LOG=str(self.log))

    def tearDown(self):
        self.tmp.cleanup()

    def run_launcher(self, *args):
        return subprocess.run([BINARY, '--force-animation', '--video', str(self.video), '--app', str(self.app), *args], env=self.env, capture_output=True, text=True, timeout=25)

    def await_log(self):
        for _ in range(100):
            if self.log.exists():
                return self.log.read_text()
            time.sleep(.01)
        self.fail('App was not started')

    def test_animation_then_app_preserves_literal_arguments(self):
        self.assertEqual(self.run_launcher('--', 'codex://test', '$(not-a-command)').returncode, 0)
        self.assertEqual(self.await_log(), 'codex://test\n$(not-a-command)\n')

    def test_player_failure_still_launches_app(self):
        self.player.write_text('#!/bin/sh\nexit 7\n')
        self.assertEqual(self.run_launcher().returncode, 0)
        self.await_log()

    def test_missing_video_still_launches_app(self):
        self.video.unlink()
        self.assertEqual(self.run_launcher().returncode, 0)
        self.await_log()

    def test_preview_does_not_launch_app(self):
        self.assertEqual(self.run_launcher('--preview').returncode, 0)
        self.assertFalse(self.log.exists())

    def test_failed_preview_returns_failure(self):
        self.player.write_text('#!/bin/sh\nexit 7\n')
        self.assertNotEqual(self.run_launcher('--preview').returncode, 0)
        self.assertFalse(self.log.exists())

    def test_wayland_session_uses_native_sdl_backend(self):
        self.env.pop('SDL_VIDEODRIVER', None)
        self.env.update(XDG_SESSION_TYPE='wayland', WAYLAND_DISPLAY='test-wayland')
        self.player.write_text('#!/bin/sh\n[ "$SDL_VIDEODRIVER" = wayland ]\n')
        self.assertEqual(self.run_launcher('--preview').returncode, 0)

    def test_concurrent_preview_is_rejected(self):
        self.player.write_text('#!/bin/sh\nexec /usr/bin/sleep 1\n')
        command = [BINARY, '--preview', '--video', str(self.video)]
        first = subprocess.Popen(command, env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            # Wait for the lock holder rather than relying on a fixed scheduling delay.
            import fcntl
            lock_path = self.root / 'codex-dragon-launch.lock'
            acquired = False
            for _ in range(100):
                if lock_path.exists():
                    with lock_path.open('r') as lock:
                        try:
                            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        except BlockingIOError:
                            acquired = True
                if acquired:
                    break
                time.sleep(.01)
            self.assertTrue(acquired)
            second = self.run_launcher('--preview')
            self.assertNotEqual(second.returncode, 0)
        finally:
            first.wait(timeout=3)

    def test_hung_player_times_out_then_launches(self):
        self.player.write_text('#!/bin/sh\nexec /usr/bin/sleep 60\n')
        started = time.monotonic()
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0)
        self.assertLess(time.monotonic() - started, 23)
        self.assertIn('timed out', result.stderr)
        self.await_log()

if __name__ == '__main__':
    unittest.main(verbosity=2)
