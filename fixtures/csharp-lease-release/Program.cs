using Rusty.Engine;

List<(Action Commit, Action Rollback)> pending = [];
bool terminal = false;
Action<Action, Action> stage = (commit, rollback) => pending.Add((commit, rollback));
Func<bool> isTerminal = () => terminal;
int appearanceDestroys = 0;
int cameraDestroys = 0;
int uiDestroys = 0;
int dynamicsDestroys = 0;
Appearance appearance = new(new AppearanceHandle(101), () => appearanceDestroys++, isTerminal, stage);
Camera camera = new(new CameraHandle(102), () => cameraDestroys++, isTerminal, stage);
UiStream ui = new(new UiStreamHandle(103), () => uiDestroys++, isTerminal, stage);
DynamicsWorld world = new(new DynamicsWorldHandle(104), () => dynamicsDestroys++, isTerminal, stage);

appearance.Dispose();
camera.Dispose();
ui.Dispose();
world.Dispose();
appearance.Dispose();
camera.Dispose();
ui.Dispose();
world.Dispose();
Require(pending.Count == 4 && appearanceDestroys == 1 && cameraDestroys == 1 && uiDestroys == 1 && dynamicsDestroys == 1,
    "commit-aware leases did not stage exactly one release per owner");
foreach ((Action _, Action rollback) in pending) rollback();
pending.Clear();

appearance.Dispose();
camera.Dispose();
ui.Dispose();
world.Dispose();
Require(pending.Count == 4 && appearanceDestroys == 2 && cameraDestroys == 2 && uiDestroys == 2 && dynamicsDestroys == 2,
    "rolled back leases were not retryable across service families");

bool nativeFailure = true;
int failureAttempts = 0;
Appearance failing = new(new AppearanceHandle(105), () =>
{
    failureAttempts++;
    if (nativeFailure) throw new InvalidOperationException("injected native destroy failure");
}, isTerminal, stage);
try
{
    failing.Dispose();
    throw new InvalidOperationException("injected destroy failure did not propagate");
}
catch (InvalidOperationException error) when (error.Message == "injected native destroy failure")
{
}
nativeFailure = false;
failing.Dispose();
Require(failureAttempts == 2 && pending.Count == 5, "native destroy failure did not leave a lease live for retry");

foreach ((Action commit, Action _) in pending) commit();
pending.Clear();
appearance.Dispose();
camera.Dispose();
ui.Dispose();
world.Dispose();
failing.Dispose();
Require(appearanceDestroys == 2 && cameraDestroys == 2 && uiDestroys == 2 && dynamicsDestroys == 2 && failureAttempts == 2,
    "committed lease releases were not locally final");

terminal = true;
Appearance terminalAppearance = new(new AppearanceHandle(106), () => appearanceDestroys++, isTerminal, stage);
terminalAppearance.Dispose();
Require(appearanceDestroys == 2 && pending.Count == 0, "terminal product teardown attempted a staged native release");

static void Require(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}
