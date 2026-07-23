import { ProjectDoorRuntime } from "./runtime.js";
import { decideSecurityDoorWave, securityDoorState } from "./security-door.js";

const runtime = ProjectDoorRuntime.create(securityDoorState(3));

try {
  const interaction = runtime.beginInteraction();
  const opened = runtime.apply(decideSecurityDoorWave(interaction));
  runtime.advanceBy(2);
  const due = runtime.advanceBy(1);
  if (due === null) {
    throw new Error("expected the stable close message at tick 3");
  }
  const closed = runtime.apply(decideSecurityDoorWave(due));

  console.log(
    JSON.stringify(
      {
        scenario: "timed-security-door",
        bridge: runtime.bridgeStats(),
        opened: {
          revision: opened.revisionAfter,
          engineFacts: opened.engineFacts.length,
          projectFacts: opened.projectFacts.map((fact) => fact.kind),
          pendingMessages: opened.pendingMessageCount,
        },
        closed: {
          tick: closed.tick,
          revision: closed.revisionAfter,
          engineFacts: closed.engineFacts.length,
          projectFacts: closed.projectFacts.map((fact) => fact.kind),
          pendingMessages: closed.pendingMessageCount,
          translation: closed.projection[0]?.translation ?? null,
        },
      },
      null,
      2,
    ),
  );
} finally {
  runtime.close();
}
