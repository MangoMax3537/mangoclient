// What the window background engine is showing (or fading to) right now. The main engine publishes it; a mirroring engine
// (the background picker's Auto preview) follows it, so the preview always shows the same image as the real background.
import type { ResolvedBackground } from './instanceIcon';

export const bgNow = $state<{ res: ResolvedBackground | null }>({ res: null });
