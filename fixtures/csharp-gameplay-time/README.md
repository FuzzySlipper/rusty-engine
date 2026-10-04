# Gameplay time proving product

An ordinary safe C# package consumer for
[gameplay time](../../docs/csharp-lifecycle.md#gameplay-time): the world moves
only as fast as the player does. Engine admits the fixed steps, delivers input
every update and renders; this product owns every rule below.

```sh
export RustyEngineFixtureSdkVersion=<matching-pair-version>
dotnet restore CsharpGameplayTime.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpGameplayTime.csproj --runtime /path/to/pair/runtime-pack
```

- **Look is free.** Mouse and right stick turn the player camera every update,
  integrated by `HostElapsedSeconds`, so aiming keeps its speed while the world
  is held or crawling. A held stick keeps turning.
- **Moving buys time.** WASD or the left stick runs the world at a rate equal
  to how far the player moves (full deflection is realtime); standing still
  holds it at exactly zero.
- **A shot buys its own duration.** Click or RB fires once per press and
  advances the world 0.35 s (21 steps at 60 Hz), then holds. The shot has a
  one-second world-time cooldown, so it cannot be repeated while held.
- **G / X** sets the idle rate to a 0.05 crawl instead of zero, to show look
  staying smooth at a very low positive rate. **T / Y** runs at realtime.
- The turret fires a slow blue orb every 1.5 s of world time
  (`SimulationScheduler`), the red drones orbit, and projectiles fly, all per
  admitted step, so all of them stop while the world holds.
- Pause/Resume at the bottom left is the menu pause (`context.lifecycle`): no
  update runs and gameplay input is discarded. Resume continues the product's
  time choice.

The HUD shows the rate (`HELD`, `ACTION` during a shot's advance), the step,
the cooldown and hits. `gameplay.observe` (live-debug) reads the same state
plus body, look, drone and projectile positions; it changes nothing and is not
needed to play.
