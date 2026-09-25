import { expect, test } from '@playwright/test';

for (const format of ['wav', 'ogg', 'opus', 'mp3', 'flac']) {
  test(`${format} emits actual samples and loops through Engine audio host`, async ({ page }) => {
    const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
    await page.goto('/browser/audio-containers.html?format=' + format);
    await page.locator('#play').click();
    const read = () => page.evaluate(() => window.__audioContainerProof.read()) as Promise<{ started: boolean; energy: number; media: { time: number; loop: boolean; src: string }[]; facts: { kind: string }[]; readout: { activeSources: number; diagnostics: unknown[] } }>;
    await expect.poll(async () => (await read()).started).toBe(true);
    await expect.poll(async () => (await read()).energy).toBeGreaterThan(0.001);
    await expect.poll(async () => (await read()).facts.some(f => f.kind === 'naturalCompletion')).toBe(true);
    await page.waitForTimeout(1600);
    const after = await read();
    expect(after.energy).toBeGreaterThan(0.001);
    expect(after.readout.activeSources).toBe(1);
    expect(after.readout.diagnostics).toEqual([]);
    if (format !== 'wav') { expect(after.media[0]!.loop).toBe(true); expect(after.media[0]!.time).toBeLessThan(1.2); }
    await page.evaluate(() => window.__audioContainerProof.stop());
    expect((await read()).media.every(m => m.src === '')).toBe(true);
    expect(errors).toEqual([]);
  });
}
