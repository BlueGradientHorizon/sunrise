#!/usr/bin/env python3
"""Record the vanilla 26.3 genpvp menu witness (settled.json, ticks.jsonl, and screenshots)."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lab  # noqa: E402
import vanilla_client  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CAPTURE = Path("/projects/.hz-tmp/genpvp-biomes/umceko/capture-dev-08550b7.jsonl")
DEFAULT_REPLAY_SERVER = Path("/projects/.hz-tmp/genpvp-biomes/scripts/replay_server.py")
DEFAULT_WITNESS_DIR = ROOT / "tools/witness/26.3/genpvp_menu"
DEFAULT_WORK_DIR = Path("/projects/.hz-tmp/genpvp-menu/witness")
DEFAULT_JDK_BIN = Path("/home/umceko/.local/jdk/jdk-25.0.5.1+2/bin")
PROBE_CLASS = "dev.sunriselab.parity.GenpvpMenuProbe"
PROBE_SOURCE = ROOT / "mods/sunrise/src/test/java/dev/sunriselab/parity/GenpvpMenuProbe.java"


def java_env():
    env = dict(os.environ)
    if DEFAULT_JDK_BIN.is_dir():
        env["PATH"] = str(DEFAULT_JDK_BIN) + os.pathsep + env.get("PATH", "")
    return env


def java_version_line(env):
    out = subprocess.check_output(["java", "--version"], env=env, text=True)
    return out.splitlines()[0].strip()


def compile_probe(classes_dir, env):
    classes_dir.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, vanilla_client.classpath()))
    subprocess.run(
        [
            "javac", "--release", "25", "-proc:none", "-nowarn",
            "-sourcepath", str(ROOT / "mods/sunrise/src/test/java"),
            "-cp", cp, "-d", str(classes_dir), str(PROBE_SOURCE),
        ],
        cwd=ROOT, env=env, check=True,
    )
    return classes_dir


def start_xvfb(min_display=170, width=1280, height=800):
    for number in range(min_display, min_display + 100):
        display = f":{number}"
        if Path(f"/tmp/.X{number}-lock").exists() or Path(f"/tmp/.X11-unix/X{number}").exists():
            continue
        proc = subprocess.Popen(
            ["Xvfb", display, "-screen", "0", f"{width}x{height}x24", "-nolisten", "tcp", "+extension", "GLX", "-noreset"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        sock = Path(f"/tmp/.X11-unix/X{number}")
        for _ in range(50):
            if proc.poll() is not None:
                break
            if sock.exists():
                return proc, display
            time.sleep(0.1)
        stop_proc(proc)
    raise RuntimeError(f"could not start Xvfb on display >= :{min_display}")


def stop_proc(proc):
    if proc is None:
        return
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path, default=DEFAULT_CAPTURE)
    parser.add_argument("--replay-server", type=Path, default=DEFAULT_REPLAY_SERVER)
    parser.add_argument("--witness-dir", type=Path, default=DEFAULT_WITNESS_DIR)
    parser.add_argument("--work-dir", type=Path, default=DEFAULT_WORK_DIR)
    parser.add_argument("--classes", type=Path, default=ROOT / ".local/mp/oracle")
    parser.add_argument("--hold", type=float, default=120.0)
    parser.add_argument("--settle-ticks", type=int, default=100)
    parser.add_argument("--mid-replay-tick", type=int, default=110)
    parser.add_argument("--min-display", type=int, default=170)
    parser.add_argument("--timeout", type=float, default=180.0)
    args = parser.parse_args()

    env = java_env()
    java_ver = java_version_line(env)
    capture_sha256 = hashlib.sha256(args.capture.read_bytes()).hexdigest()

    args.witness_dir.mkdir(parents=True, exist_ok=True)
    args.work_dir.mkdir(parents=True, exist_ok=True)
    game_dir = args.work_dir / "game"
    if game_dir.exists():
        shutil.rmtree(game_dir)
    game_dir.mkdir(parents=True, exist_ok=True)

    replay_done_file = args.work_dir / "replay_done"
    replay_done_file.unlink(missing_ok=True)
    for png in ("camera_set.png", "mid_replay.png", "settled.png"):
        (args.work_dir / png).unlink(missing_ok=True)

    settled_out = args.witness_dir / "settled.json"
    ticks_out = args.witness_dir / "ticks.jsonl"
    settled_out.unlink(missing_ok=True)
    ticks_out.unlink(missing_ok=True)

    print(f"Compiling {PROBE_SOURCE.name}...", flush=True)
    compile_probe(args.classes, env)

    port = lab.free_port()
    xvfb = None
    replay = None
    client = None
    replay_log = (args.work_dir / "replay.log").open("w", encoding="utf-8")
    client_log = (args.work_dir / "client.log").open("w", encoding="utf-8")

    try:
        xvfb, display = start_xvfb(min_display=args.min_display, width=1280, height=800)
        print(f"Started Xvfb on {display} (pid {xvfb.pid})", flush=True)

        replay_listening = threading.Event()
        replay_done = threading.Event()
        replay = subprocess.Popen(
            [sys.executable, str(args.replay_server), str(args.capture), str(port), str(args.hold)],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1,
        )

        def watch_replay():
            for line in replay.stdout:
                replay_log.write(line)
                replay_log.flush()
                s = line.strip()
                if s == "listening":
                    replay_listening.set()
                elif s.startswith("replayed"):
                    replay_done_file.write_text(s + "\n", encoding="utf-8")
                    replay_done.set()

        threading.Thread(target=watch_replay, daemon=True).start()
        if not replay_listening.wait(30):
            raise RuntimeError("replay_server.py did not report listening within 30s")
        print(f"Replay server listening on 127.0.0.1:{port} (pid {replay.pid})", flush=True)

        client_env = vanilla_client.environment()
        client_env["PATH"] = env["PATH"]
        for key in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "EGL_PLATFORM"):
            client_env.pop(key, None)
        client_env["DISPLAY"] = display
        client_env["XDG_SESSION_TYPE"] = "x11"
        client_env["LIBGL_ALWAYS_SOFTWARE"] = "1"
        client_env["__GLX_VENDOR_LIBRARY_NAME"] = "mesa"
        mesa_egl = Path("/usr/share/glvnd/egl_vendor.d/50_mesa.json")
        if mesa_egl.exists():
            client_env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(mesa_egl)
        lvp = Path("/usr/share/vulkan/icd.d/lvp_icd.json")
        if lvp.exists():
            client_env["VK_ICD_FILENAMES"] = str(lvp)

        # In options.txt, fov is stored normalized as (degrees - 70.0) / 40.0, so 70 degrees is 0.0.
        extra_options = {
            "renderDistance": "2",
            "simulationDistance": "5",
            "guiScale": "2",
            "fov": "0.0",
        }
        vanilla_client.write_options(game_dir, extra_options)

        witness_cmd = "python3 tools/genpvp_menu_witness.py"
        client_args = argparse.Namespace(
            name="Guest",
            join=f"127.0.0.1:{port}",
            world=None,
            game_dir=game_dir,
            assets=None,
            heap="3G",
            width=1280,
            height=800,
            lan_port=None,
            probe_out=settled_out,
            probe_class=PROBE_CLASS,
            jvm_arg=[
                f"-Dsunrise.settledOut={settled_out}",
                f"-Dsunrise.ticksOut={ticks_out}",
                f"-Dsunrise.screenshotDir={args.work_dir}",
                f"-Dsunrise.replayDoneFile={replay_done_file}",
                f"-Dsunrise.settleTicks={args.settle_ticks}",
                f"-Dsunrise.midReplayTick={args.mid_replay_tick}",
                f"-Dsunrise.captureSha256={capture_sha256}",
                f"-Dsunrise.witnessCommand={witness_cmd}",
                f"-Dsunrise.javaVersion={java_ver}",
            ],
            classes=args.classes,
            backend=None,
        )
        cmd = vanilla_client.command(client_args)
        client = subprocess.Popen(
            cmd, cwd=game_dir, env=client_env,
            stdin=subprocess.DEVNULL, stdout=client_log, stderr=subprocess.STDOUT,
        )
        print(f"Started vanilla 26.3 client (pid {client.pid})", flush=True)

        deadline = time.time() + args.timeout
        while time.time() < deadline:
            if client.poll() is not None:
                break
            time.sleep(0.5)

        if client.poll() is None:
            raise TimeoutError(f"vanilla client did not exit within {args.timeout}s")

        if not settled_out.is_file() or not ticks_out.is_file():
            raise RuntimeError(f"client exited {client.returncode} without producing witness files; see {args.work_dir / 'client.log'}")

        for png in ("camera_set.png", "mid_replay.png", "settled.png"):
            p = args.work_dir / png
            if not p.is_file() or p.stat().st_size == 0:
                raise RuntimeError(f"missing screenshot {p}")

        print(f"Witness recorded: {settled_out}, {ticks_out}", flush=True)
        print(f"Screenshots: {args.work_dir / 'camera_set.png'}, {args.work_dir / 'mid_replay.png'}, {args.work_dir / 'settled.png'}", flush=True)
    finally:
        stop_proc(client)
        stop_proc(replay)
        stop_proc(xvfb)
        replay_log.close()
        client_log.close()


if __name__ == "__main__":
    main()
