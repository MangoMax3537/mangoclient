package gg.mangoclient.mangoconfig;

import gg.mangoclient.mangoconfig.compat.Compat;
import gg.mangoclient.mangoconfig.compat.CompatScreen;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerWarningScreen;
import net.minecraft.client.gui.screen.option.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screen.option.LanguageOptionsScreen;
import net.minecraft.client.gui.screen.option.OptionsScreen;
import net.minecraft.client.gui.screen.world.SelectWorldScreen;
import net.minecraft.client.realms.gui.screen.RealmsMainScreen;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import net.minecraft.util.Util;

import java.io.DataInputStream;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * MangoClient's home screen, shown in place of vanilla's title screen.
 *
 * The blurred panorama behind it is vanilla's own menu background; on top sit
 * the pixel mango and wordmark, the main buttons down the middle, the player
 * and the small screens in a bar top right, and the Discord card top left.
 *
 * Icons are custom high-resolution minimalist Lucide icons rendered cleanly
 * via Compat.drawTexture across all supported Minecraft versions.
 */
public class MangoTitleScreen extends CompatScreen {
	private static final String DISCORD = "https://discord.gg/XWNvJTZEWB";

	private static final int BUTTON_W = 176;
	private static final int BUTTON_H = 24;
	private static final int BUTTON_GAP = 6;
	private static final int QUIT_W = 110;
	private static final int QUIT_H = 20;
	private static final int BAR_H = 22;
	private static final int EDGE = 8;
	/** GUI pixels per pixel of the mango: big, but still on the pixel grid. */
	private static final int LOGO_SCALE = 2;

	/** Translucent enough that the panorama glows through, as on the launcher. */
	private static final int PLATE = 0xC8141210;
	private static final int PLATE_HOVER = 0xE02A2621;

	/** The mango as runs of {x, y, length, argb}, read once from logo.bin. */
	private static int[][] logo;
	private static int logoW;
	private static int logoH;

	private final long openedAt = System.currentTimeMillis();
	private long lastFrame;
	private float dt;
	private final Map<String, Float> anims = new HashMap<>();

	/** What can be clicked this frame; rebuilt as it is drawn. */
	private record Hit(int x, int y, int w, int h, Runnable action) {
	}

	private final List<Hit> hits = new ArrayList<>();
	/** Opacity of everything this frame, for the fade in. */
	private float alpha = 1f;

	public MangoTitleScreen() {
		super(Text.literal("MangoClient"));
	}

	@Override
	public boolean shouldCloseOnEsc() {
		return false;
	}

	// --- logo ----------------------------------------------------------------

	private static void loadLogo() {
		if (logo != null) return;
		List<int[]> runs = new ArrayList<>();
		try (InputStream raw = MangoTitleScreen.class.getResourceAsStream("/assets/mangoconfig/logo.bin")) {
			if (raw != null) {
				DataInputStream in = new DataInputStream(raw);
				logoW = in.readInt();
				logoH = in.readInt();
				for (int y = 0; y < logoH; y++) {
					int start = 0;
					int colour = 0;
					for (int x = 0; x <= logoW; x++) {
						int c = x < logoW ? in.readInt() : 0;
						if (x == 0) {
							colour = c;
							continue;
						}
						if (c != colour) {
							if ((colour >>> 24) >= 0x80) runs.add(new int[]{start, y, x - start, colour | 0xFF000000});
							start = x;
							colour = c;
						}
					}
				}
			}
		} catch (Exception e) {
			// No mango, then; the wordmark still says whose screen this is.
		}
		logo = runs.toArray(new int[0][]);
	}

	// --- drawing helpers -----------------------------------------------------

	private int a(int argb) {
		int alphaIn = (argb >>> 24) & 0xFF;
		return Theme.withAlpha(argb, Math.round(alphaIn * alpha));
	}

	/** Text below this opacity would be drawn fully opaque by older versions, so it is skipped. */
	private boolean textVisible() {
		return alpha > 0.04f;
	}

	private void text(DrawContext ctx, Text text, int x, int y, int colour, boolean shadow) {
		if (textVisible()) ctx.drawText(this.textRenderer, text, x, y, a(colour), shadow);
	}

	private void text(DrawContext ctx, String text, int x, int y, int colour, boolean shadow) {
		if (textVisible()) ctx.drawText(this.textRenderer, text, x, y, a(colour), shadow);
	}

	/** A plate with its corners cut by a pixel, which reads as rounded at GUI scale. */
	private void plate(DrawContext ctx, int x, int y, int w, int h, int colour) {
		int c = a(colour);
		ctx.fill(x + 1, y, x + w - 1, y + h, c);
		ctx.fill(x, y + 1, x + 1, y + h - 1, c);
		ctx.fill(x + w - 1, y + 1, x + w, y + h - 1, c);
	}

	/** The lit top edge every plate on the launcher has. */
	private void topEdge(DrawContext ctx, int x, int y, int w, int colour) {
		ctx.fill(x + 1, y, x + w - 1, y + 1, a(colour));
	}



	private float anim(String key, boolean on, float speed) {
		float v = anims.getOrDefault(key, on ? 1f : 0f);
		v += ((on ? 1f : 0f) - v) * Math.min(1f, dt * speed);
		anims.put(key, v);
		return v;
	}

	private static float ease(float t) {
		float u = 1f - Math.max(0f, Math.min(1f, t));
		return 1f - u * u * u;
	}

	private static boolean inside(int mx, int my, int x, int y, int w, int h) {
		return mx >= x && mx < x + w && my >= y && my < y + h;
	}

	// --- rendering -----------------------------------------------------------

	@Override
	public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
		// Vanilla's menu background first: up to 1.21.5 Screen.render draws it,
		// from 1.21.6 it has already been drawn before render is called.
		super.render(ctx, mouseX, mouseY, delta);

		long now = System.currentTimeMillis();
		dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1000f);
		lastFrame = now;
		alpha = ease((now - openedAt) / 450f);
		hits.clear();

		drawCentre(ctx, mouseX, mouseY, now);
		drawTopBar(ctx, mouseX, mouseY);
		drawDiscord(ctx, mouseX, mouseY);
		drawVersion(ctx);
	}

	private void drawCentre(DrawContext ctx, int mouseX, int mouseY, long now) {
		loadLogo();
		MinecraftClient mc = MinecraftClient.getInstance();

		int logoBlock = logoH * LOGO_SCALE + 6 + 18;
		int columnH = logoBlock + 14 + 4 * BUTTON_H + 3 * BUTTON_GAP + 16 + QUIT_H;
		// A short window shrinks the whole column rather than letting it run
		// under the bars at the top and the version line at the bottom.
		float fit = Math.min(1f, (this.height - 2 * (BAR_H + EDGE + 4)) / (float) columnH);
		int cx = this.width / 2;
		float slide = (1f - alpha) * 8f;
		int top = Math.round((this.height - columnH * fit) / 2f);

		Compat.push(ctx);
		Compat.translate(ctx, cx, top + slide);
		Compat.scale(ctx, fit, fit);
		// From here on, x is relative to the centre line and y to the column top.
		int mx = Math.round((mouseX - cx) / fit);
		int my = Math.round((mouseY - top - slide) / fit);

		// The mango, gently bobbing, then the wordmark under it.
		float bob = (float) Math.sin(now / 900.0) * 1.5f;
		Compat.push(ctx);
		Compat.translate(ctx, -logoW * LOGO_SCALE / 2f, bob);
		Compat.scale(ctx, LOGO_SCALE, LOGO_SCALE);
		for (int[] run : logo) ctx.fill(run[0], run[1], run[0] + run[2], run[1] + 1, a(run[3]));
		Compat.pop(ctx);

		Text mango = Text.literal("MANGO").formatted(Formatting.BOLD);
		Text client = Text.literal("CLIENT").formatted(Formatting.BOLD);
		int wordW = this.textRenderer.getWidth(mango) + this.textRenderer.getWidth(client);
		Compat.push(ctx);
		Compat.translate(ctx, 0, logoH * LOGO_SCALE + 6);
		Compat.scale(ctx, 2f, 2f);
		int wx = -wordW / 2;
		text(ctx, mango, wx, 0, Theme.BRAND, true);
		text(ctx, client, wx + this.textRenderer.getWidth(mango), 0, Theme.TEXT, true);
		Compat.pop(ctx);

		int y = logoBlock + 14;
		y = button(ctx, "single", Text.translatable("menu.singleplayer"), y, mx, my, false,
			() -> mc.setScreen(new SelectWorldScreen(this)));
		y = button(ctx, "multi", Text.translatable("menu.multiplayer"), y, mx, my, false,
			() -> mc.setScreen(mc.options.skipMultiplayerWarning
				? new MultiplayerScreen(this) : new MultiplayerWarningScreen(this)));
		y = button(ctx, "options", Text.translatable("menu.options"), y, mx, my, false,
			() -> mc.setScreen(new OptionsScreen(this, mc.options)));
		y = button(ctx, "mangoconfig", Text.literal("MANGOCONFIG").formatted(Formatting.BOLD), y, mx, my,
			true, () -> mc.setScreen(new MangoConfigScreen()));

		// Quit sits apart, quieter, so it is never the button a hand lands on.
		int qy = y - BUTTON_GAP + 16;
		boolean hot = inside(mx, my, -QUIT_W / 2, qy, QUIT_W, QUIT_H);
		float h = ease(anim("quit", hot, 12f));
		plate(ctx, -QUIT_W / 2, qy, QUIT_W, QUIT_H, Theme.withAlpha(Theme.BRAND, Math.round(0x30 + 0x30 * h)));
		Text quit = Text.translatable("menu.quit").copy().formatted(Formatting.BOLD);
		String label = quit.getString().toUpperCase(Locale.ROOT);
		Text upper = Text.literal(label).formatted(Formatting.BOLD);
		text(ctx, upper, -this.textRenderer.getWidth(upper) / 2, qy + (QUIT_H - 8) / 2,
			Theme.mix(Theme.BRAND, Theme.BRAND_HOVER, h), false);
		hits.add(new Hit(Math.round(cx - QUIT_W / 2f * fit), Math.round(top + slide + qy * fit),
			Math.round(QUIT_W * fit), Math.round(QUIT_H * fit), mc::scheduleStop));

		Compat.pop(ctx);

		// The column's own clicks were recorded in its local space; map them back.
		for (int i = 0; i < pending.size(); i++) {
			Hit local = pending.get(i);
			hits.add(new Hit(Math.round(cx + local.x() * fit), Math.round(top + slide + local.y() * fit),
				Math.round(local.w() * fit), Math.round(local.h() * fit), local.action()));
		}
		pending.clear();
	}

	/** Column buttons, in the column's local space until drawCentre maps them. */
	private final List<Hit> pending = new ArrayList<>();

	private int button(DrawContext ctx, String key, Text label, int y, int mx, int my,
			boolean primary, Runnable action) {
		int x = -BUTTON_W / 2;
		boolean hot = inside(mx, my, x, y, BUTTON_W, BUTTON_H);
		float h = ease(anim(key, hot, 12f));

		if (primary) {
			// The one orange button, like the launcher's Play: a warm gradient
			// that lifts a shade under the cursor.
			int topColour = Theme.mix(Theme.BRAND, Theme.BRAND_HOVER, h);
			int c1 = a(topColour);
			int c2 = a(Theme.mix(Theme.BRAND_DEEP, Theme.BRAND, h * 0.5f));
			ctx.fillGradient(x + 1, y, x + BUTTON_W - 1, y + BUTTON_H, c1, c2);
			ctx.fillGradient(x, y + 1, x + 1, y + BUTTON_H - 1, c1, c2);
			ctx.fillGradient(x + BUTTON_W - 1, y + 1, x + BUTTON_W, y + BUTTON_H - 1, c1, c2);
			topEdge(ctx, x, y, BUTTON_W, 0x40FFFFFF);
		} else {
			plate(ctx, x, y, BUTTON_W, BUTTON_H, h > 0.01f ? lerp(PLATE, PLATE_HOVER, h) : PLATE);
			topEdge(ctx, x, y, BUTTON_W, Theme.BTN_BORDER);
			// An orange tick grows in on the left under the cursor.
			int tick = Math.round(h * (BUTTON_H - 8));
			if (tick > 0) {
				ctx.fill(x + 1, y + (BUTTON_H - tick) / 2, x + 3, y + (BUTTON_H + tick) / 2, a(Theme.BRAND));
			}
		}

		int labelW = this.textRenderer.getWidth(label);
		int iconSize = 14;
		int contentW = iconSize + 6 + labelW;
		int ix = -contentW / 2;
		int textColour = primary ? Theme.TEXT : Theme.mix(Theme.TEXT_2, Theme.TEXT, h);
		Compat.drawTexture(ctx, Icons.homeIcon(key, primary || hot), ix, y + (BUTTON_H - iconSize) / 2, iconSize, iconSize);
		text(ctx, label, ix + iconSize + 6, y + (BUTTON_H - 8) / 2, textColour, primary);

		pending.add(new Hit(x, y, BUTTON_W, BUTTON_H, action));
		return y + BUTTON_H + BUTTON_GAP;
	}

	private static int lerp(int from, int to, float t) {
		int a = Math.round(((from >>> 24) & 0xFF) * (1 - t) + ((to >>> 24) & 0xFF) * t);
		return Theme.withAlpha(Theme.mix(from | 0xFF000000, to | 0xFF000000, t), a);
	}

	private void drawTopBar(DrawContext ctx, int mouseX, int mouseY) {
		MinecraftClient mc = MinecraftClient.getInstance();
		int y = EDGE;
		int x = this.width - EDGE;
		String hover = null;
		int hoverX = 0;

		// Right to left: the small screens, then the player chip.
		Object[][] buttons = {
			{"Realms", "realms", (Runnable) () -> mc.setScreen(new RealmsMainScreen(this))},
			{"Accessibility", "access",
				(Runnable) () -> mc.setScreen(new AccessibilityOptionsScreen(this, mc.options))},
			{"Language", "globe",
				(Runnable) () -> mc.setScreen(new LanguageOptionsScreen(this, mc.options, mc.getLanguageManager()))},
			{"Options", "options", (Runnable) () -> mc.setScreen(new OptionsScreen(this, mc.options))},
			{"MangoConfig", "mangoconfig", (Runnable) () -> mc.setScreen(new MangoConfigScreen())},
		};
		for (Object[] b : buttons) {
			String name = (String) b[0];
			String iconKey = (String) b[1];
			x -= BAR_H;
			boolean hot = inside(mouseX, mouseY, x, y, BAR_H, BAR_H);
			float h = ease(anim("bar:" + name, hot, 12f));
			plate(ctx, x, y, BAR_H, BAR_H, lerp(PLATE, PLATE_HOVER, h));
			topEdge(ctx, x, y, BAR_H, Theme.BTN_BORDER);
			int iconSize = 14;
			Compat.drawTexture(ctx, Icons.homeIcon(iconKey, hot), x + (BAR_H - iconSize) / 2, y + (BAR_H - iconSize) / 2, iconSize, iconSize);
			hits.add(new Hit(x, y, BAR_H, BAR_H, (Runnable) b[2]));
			if (hot) {
				hover = name;
				hoverX = x + BAR_H / 2;
			}
			x -= 4;
		}

		// The player: face and name, the way the launcher's account chip reads.
		String user = mc.getSession().getUsername().toUpperCase(Locale.ROOT);
		Text name = Text.literal(user).formatted(Formatting.BOLD);
		int chipW = 6 + 12 + 6 + this.textRenderer.getWidth(name) + 8;
		x -= chipW;
		plate(ctx, x, y, chipW, BAR_H, PLATE);
		topEdge(ctx, x, y, chipW, Theme.BTN_BORDER);
		if (alpha > 0.5f) {
			try {
				Compat.drawFace(ctx, mc, x + 6, y + (BAR_H - 12) / 2, 12);
			} catch (Throwable ignored) {
				// A skin that cannot be fetched leaves just the name.
			}
		}
		text(ctx, name, x + 6 + 12 + 6, y + (BAR_H - 8) / 2, Theme.TEXT, false);

		if (hover != null) tooltip(ctx, hover, hoverX, y + BAR_H + 4);
	}

	private void tooltip(DrawContext ctx, String label, int centreX, int y) {
		int w = this.textRenderer.getWidth(label) + 10;
		int x = Math.min(this.width - EDGE - w, centreX - w / 2);
		plate(ctx, x, y, w, 14, 0xF0191614);
		text(ctx, label, x + 5, y + 3, Theme.TEXT_2, false);
	}

	private void drawDiscord(DrawContext ctx, int mouseX, int mouseY) {
		String title = "Join our Discord";
		String link = DISCORD.replace("https://", "");
		int iconSize = 16;
		int w = 8 + iconSize + 8 + Math.max(this.textRenderer.getWidth(title),
			this.textRenderer.getWidth(link)) + 10;
		int h = 30;
		int x = EDGE;
		int y = EDGE;
		boolean hot = inside(mouseX, mouseY, x, y, w, h);
		float e = ease(anim("discord", hot, 12f));
		plate(ctx, x, y, w, h, lerp(PLATE, PLATE_HOVER, e));
		topEdge(ctx, x, y, w, Theme.BTN_BORDER);
		Compat.drawTexture(ctx, Icons.homeIcon("discord", hot), x + 8, y + (h - iconSize) / 2, iconSize, iconSize);
		int tx = x + 8 + iconSize + 8;
		text(ctx, title, tx, y + 6, Theme.TEXT, false);
		text(ctx, link, tx, y + 16, Theme.mix(Theme.TEXT_3, Theme.BRAND_HOVER, e), false);
		hits.add(new Hit(x, y, w, h, () -> Util.getOperatingSystem().open(DISCORD)));
	}

	private void drawVersion(DrawContext ctx) {
		String mc = FabricLoader.getInstance().getModContainer("minecraft")
			.map(m -> m.getMetadata().getVersion().getFriendlyString()).orElse("");
		String mod = FabricLoader.getInstance().getModContainer("mangoconfig")
			.map(m -> m.getMetadata().getVersion().getFriendlyString().split("\\+")[0]).orElse("");
		String line = "MangoClient  ·  Minecraft " + mc + "  ·  MangoConfig " + mod;
		text(ctx, line, (this.width - this.textRenderer.getWidth(line)) / 2, this.height - 14, 0xB0A89F92, false);
	}

	// --- input ---------------------------------------------------------------

	@Override
	protected boolean onMouseDown(double mx, double my) {
		for (Hit hit : hits) {
			if (inside((int) Math.floor(mx), (int) Math.floor(my), hit.x(), hit.y(), hit.w(), hit.h())) {
				Compat.clickSound(MinecraftClient.getInstance());
				hit.action().run();
				return true;
			}
		}
		return false;
	}
}
