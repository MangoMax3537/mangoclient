package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.Click;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.input.KeyInput;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/**
 * The settings screen: modules down the left, the selected module's settings
 * down the right. Drawn by hand rather than with vanilla widgets so it can
 * carry MangoClient's own look.
 */
public class MangoConfigScreen extends Screen {
	private static final int PANEL_W = 440;
	private static final int PANEL_H = 250;
	private static final int LIST_W = 150;
	private static final int ROW_H = 22;
	private static final int HEADER_H = 34;

	private int left;
	private int top;
	private int selected;
	/** Index of the range option being dragged, or -1. */
	private int draggingOption = -1;

	public MangoConfigScreen() {
		super(Text.literal(MangoConfig.NAME));
	}

	@Override
	protected void init() {
		left = (this.width - PANEL_W) / 2;
		top = (this.height - PANEL_H) / 2;
	}

	private List<HudModule> modules() {
		return MangoConfig.modules();
	}

	private HudModule current() {
		List<HudModule> all = modules();
		if (all.isEmpty()) return null;
		return all.get(Math.max(0, Math.min(all.size() - 1, selected)));
	}

	@Override
	public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
		ctx.fill(0, 0, this.width, this.height, Theme.SCRIM);

		ctx.fill(left, top, left + PANEL_W, top + PANEL_H, Theme.SURFACE_1);
		ctx.fill(left, top, left + PANEL_W, top + HEADER_H, Theme.SURFACE_2);
		ctx.fill(left, top + HEADER_H - 1, left + PANEL_W, top + HEADER_H, Theme.SURFACE_4);

		ctx.drawText(this.textRenderer, MangoConfig.NAME, left + 12, top + 8, Theme.BRAND, false);
		ctx.drawText(this.textRenderer, "MangoClient", left + 12, top + 20, Theme.TEXT_3, false);

		drawHudSwitch(ctx);
		drawEditorButton(ctx, mouseX, mouseY);

		ctx.fill(left, top + HEADER_H, left + LIST_W, top + PANEL_H, Theme.SURFACE_2);
		drawModuleList(ctx, mouseX, mouseY);
		drawOptions(ctx);

		super.render(ctx, mouseX, mouseY, delta);
	}

	// --- header ------------------------------------------------------------

	private int[] hudSwitchRect() {
		return new int[]{left + PANEL_W - 52, top + 10, 40, 14};
	}

	private void drawHudSwitch(DrawContext ctx) {
		int[] r = hudSwitchRect();
		boolean on = MangoConfig.config().hudEnabled;
		ctx.fill(r[0], r[1], r[0] + r[2], r[1] + r[3], on ? Theme.BRAND : Theme.SURFACE_4);
		int knob = on ? r[0] + r[2] - 12 : r[0] + 2;
		ctx.fill(knob, r[1] + 2, knob + 10, r[1] + r[3] - 2, on ? Theme.BRAND_FG : Theme.TEXT_3);

		String label = on ? "HUD an" : "HUD aus";
		int w = this.textRenderer.getWidth(label);
		ctx.drawText(this.textRenderer, label, r[0] - w - 8, r[1] + 3, on ? Theme.TEXT : Theme.TEXT_3, false);
	}

	private int[] editorRect() {
		return new int[]{left + PANEL_W - 132, top + PANEL_H - 26, 120, 18};
	}

	private void drawEditorButton(DrawContext ctx, int mouseX, int mouseY) {
		int[] r = editorRect();
		boolean hot = inside(mouseX, mouseY, r);
		ctx.fill(r[0], r[1], r[0] + r[2], r[1] + r[3], hot ? Theme.BRAND_HOVER : Theme.BRAND);
		String label = "HUD anordnen";
		int w = this.textRenderer.getWidth(label);
		ctx.drawText(this.textRenderer, label, r[0] + (r[2] - w) / 2, r[1] + 5, Theme.BRAND_FG, false);
	}

	// --- module list -------------------------------------------------------

	private void drawModuleList(DrawContext ctx, int mouseX, int mouseY) {
		List<HudModule> all = modules();
		for (int i = 0; i < all.size(); i++) {
			HudModule module = all.get(i);
			int y = top + HEADER_H + 6 + i * ROW_H;
			boolean active = i == selected;
			boolean hot = mouseX >= left && mouseX < left + LIST_W && mouseY >= y && mouseY < y + ROW_H;

			if (active) {
				ctx.fill(left, y, left + LIST_W, y + ROW_H, Theme.SURFACE_3);
				ctx.fill(left, y, left + 2, y + ROW_H, Theme.BRAND);
			} else if (hot) {
				ctx.fill(left, y, left + LIST_W, y + ROW_H, Theme.SURFACE_3);
			}

			ctx.drawText(this.textRenderer, module.name, left + 12, y + 7,
				active ? Theme.TEXT : Theme.TEXT_2, false);

			int dot = left + LIST_W - 16;
			ctx.fill(dot, y + 9, dot + 5, y + 14, module.enabled ? Theme.OK : Theme.SURFACE_4);
		}
	}

	// --- options -----------------------------------------------------------

	private int optionsX() {
		return left + LIST_W + 14;
	}

	private int optionsWidth() {
		return PANEL_W - LIST_W - 28;
	}

	private int rowY(int index) {
		return top + HEADER_H + 10 + index * 26;
	}

	/** The shared settings first, then whatever the module added. */
	private List<Option> optionList(HudModule module) {
		List<Option> rows = new ArrayList<>();
		rows.add(module.scale);
		rows.add(module.colour);
		rows.add(module.shadow);
		rows.add(module.background);
		rows.add(module.backgroundColour);
		rows.addAll(module.options);
		return rows;
	}

	private void drawOptions(DrawContext ctx) {
		HudModule module = current();
		if (module == null) return;

		int x = optionsX();
		int w = optionsWidth();

		int y = rowY(0);
		ctx.drawText(this.textRenderer, module.name, x, y, Theme.TEXT, false);
		ctx.drawText(this.textRenderer, module.description, x, y + 11, Theme.TEXT_3, false);
		drawToggle(ctx, x + w - 26, y - 2, module.enabled);

		List<Option> rows = optionList(module);
		for (int i = 0; i < rows.size(); i++) {
			drawOptionRow(ctx, rows.get(i), x, rowY(i + 1) + 6, w);
		}
	}

	private void drawOptionRow(DrawContext ctx, Option option, int x, int y, int w) {
		ctx.drawText(this.textRenderer, option.label, x, y + 3, Theme.TEXT_2, false);

		if (option instanceof Option.Bool b) {
			drawToggle(ctx, x + w - 26, y, b.value);
		} else if (option instanceof Option.Range r) {
			int trackX = x + w - 96;
			int trackY = y + 5;
			ctx.fill(trackX, trackY, trackX + 70, trackY + 3, Theme.SURFACE_4);
			int fill = Math.round(70 * r.fraction());
			ctx.fill(trackX, trackY, trackX + fill, trackY + 3, Theme.BRAND);
			int knob = trackX + Math.max(0, fill - 2);
			ctx.fill(knob, trackY - 3, knob + 4, trackY + 6, Theme.BRAND_HOVER);
			String value = r.display();
			ctx.drawText(this.textRenderer, value, x + w - this.textRenderer.getWidth(value), y + 3, Theme.TEXT_3, false);
		} else if (option instanceof Option.Colour c) {
			int sw = x + w - 20;
			ctx.fill(sw - 1, y - 1, sw + 15, y + 11, Theme.SURFACE_4);
			ctx.fill(sw, y, sw + 14, y + 10, 0xFF000000 | (c.value & 0x00FFFFFF));
		} else if (option instanceof Option.Choice ch) {
			String value = ch.value();
			int vw = this.textRenderer.getWidth(value);
			ctx.drawText(this.textRenderer, value, x + w - vw, y + 3, Theme.BRAND, false);
		}
	}

	private void drawToggle(DrawContext ctx, int x, int y, boolean on) {
		ctx.fill(x, y, x + 22, y + 11, on ? Theme.BRAND : Theme.SURFACE_4);
		int knob = on ? x + 13 : x + 2;
		ctx.fill(knob, y + 2, knob + 7, y + 9, on ? Theme.BRAND_FG : Theme.TEXT_3);
	}

	// --- input -------------------------------------------------------------

	private static boolean inside(double mx, double my, int[] r) {
		return mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
	}

	@Override
	public boolean mouseClicked(Click click, boolean doubled) {
		double mx = click.x();
		double my = click.y();

		if (inside(mx, my, hudSwitchRect())) {
			MangoConfig.config().hudEnabled = !MangoConfig.config().hudEnabled;
			MangoConfig.save();
			return true;
		}
		if (inside(mx, my, editorRect())) {
			MinecraftClient.getInstance().setScreen(new HudEditorScreen(this));
			return true;
		}

		List<HudModule> all = modules();
		if (mx >= left && mx < left + LIST_W) {
			for (int i = 0; i < all.size(); i++) {
				int y = top + HEADER_H + 6 + i * ROW_H;
				if (my >= y && my < y + ROW_H) {
					selected = i;
					return true;
				}
			}
		}

		HudModule module = current();
		if (module == null) return super.mouseClicked(click, doubled);

		int x = optionsX();
		int w = optionsWidth();

		if (inside(mx, my, new int[]{x + w - 26, rowY(0) - 2, 22, 11})) {
			module.enabled = !module.enabled;
			MangoConfig.save();
			return true;
		}

		List<Option> rows = optionList(module);
		for (int i = 0; i < rows.size(); i++) {
			int y = rowY(i + 1) + 6;
			if (my < y - 4 || my > y + 14) continue;
			Option option = rows.get(i);

			if (option instanceof Option.Bool b && inside(mx, my, new int[]{x + w - 26, y, 22, 11})) {
				b.toggle();
				MangoConfig.save();
				return true;
			}
			if (option instanceof Option.Range r && mx >= x + w - 96 && mx <= x + w - 26) {
				draggingOption = i;
				r.setFraction((mx - (x + w - 96)) / 70.0);
				return true;
			}
			if (option instanceof Option.Colour c && mx >= x + w - 21) {
				c.value = nextSwatch(c.value);
				MangoConfig.save();
				return true;
			}
			if (option instanceof Option.Choice ch && mx >= x + w - 110) {
				ch.next();
				MangoConfig.save();
				return true;
			}
		}
		return super.mouseClicked(click, doubled);
	}

	/** Colours cycle the palette; alpha is kept so background plates stay sheer. */
	private static int nextSwatch(int current) {
		int alpha = (current >>> 24) & 0xFF;
		int rgb = current & 0x00FFFFFF;
		for (int i = 0; i < Theme.SWATCHES.length; i++) {
			if ((Theme.SWATCHES[i] & 0x00FFFFFF) == rgb) {
				return (alpha << 24) | (Theme.SWATCHES[(i + 1) % Theme.SWATCHES.length] & 0x00FFFFFF);
			}
		}
		return (alpha << 24) | (Theme.SWATCHES[0] & 0x00FFFFFF);
	}

	@Override
	public boolean mouseDragged(Click click, double dx, double dy) {
		HudModule module = current();
		if (draggingOption >= 0 && module != null) {
			List<Option> rows = optionList(module);
			if (draggingOption < rows.size() && rows.get(draggingOption) instanceof Option.Range r) {
				int x = optionsX();
				int w = optionsWidth();
				r.setFraction((click.x() - (x + w - 96)) / 70.0);
				return true;
			}
		}
		return super.mouseDragged(click, dx, dy);
	}

	@Override
	public boolean mouseReleased(Click click) {
		if (draggingOption >= 0) {
			draggingOption = -1;
			MangoConfig.save();
			return true;
		}
		return super.mouseReleased(click);
	}

	@Override
	public boolean keyPressed(KeyInput input) {
		if (input.key() == GLFW.GLFW_KEY_ESCAPE) {
			this.close();
			return true;
		}
		return super.keyPressed(input);
	}

	@Override
	public void close() {
		MangoConfig.save();
		super.close();
	}
}
