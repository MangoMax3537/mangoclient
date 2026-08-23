package gg.mangoclient.mangoconfig;

import gg.mangoclient.mangoconfig.compat.Compat;
import gg.mangoclient.mangoconfig.compat.CompatScreen;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * The settings screen: categories down the left, and on the right either a
 * grid of mod cards - four to a row, like the launcher-clients this styles
 * itself after - or a list of setting rows.
 *
 * A card is the mod's face: its initial, its name, its switch. Clicking the
 * switch toggles it; clicking anywhere else opens the mod's own settings in
 * place. The search box in the header filters the cards of every category at
 * once, so a mod can be found without knowing which page it lives on.
 *
 * Setting rows lay themselves out from the right: the control claims the width
 * it needs, and the label is trimmed to whatever is left. That is what keeps a
 * long name from running out of the panel, whatever the window size.
 */
public class MangoConfigScreen extends CompatScreen {
	private static final int RAIL_W = 96;
	private static final int HEADER_H = 40;
	private static final int FOOTER_H = 30;
	private static final int ROW_H = 22;
	private static final int HINT_ROW_H = 31;
	private static final int PAD = 10;

	/** The grid: always four cards to a row, scrolling for the rest. */
	private static final int COLS = 4;
	private static final int GAP = 6;
	private static final int CARD_H = 56;

	/** Widths the controls reserve on the right of a row. */
	private static final int SWITCH_W = 22;
	private static final int TRACK_W = 64;
	private static final int VALUE_W = 46;

	private static final int SEARCH_H = 16;
	private static final int QUERY_MAX = 24;

	private int left;
	private int top;
	private int panelW;
	private int panelH;

	private int category;
	/** The card whose settings fill the panel; null while the grid shows. */
	private ModEntry detail;
	private final StringBuilder query = new StringBuilder();
	private boolean searchFocused;
	private int scroll;
	private Option.Range draggingRange;
	/** The key row waiting for a press, if any. */
	private Option.Key listeningKey;

	/**
	 * Everything that moves rather than jumps: each key eases toward 1 while
	 * its condition holds and back to 0 when it stops, and the draws read the
	 * eased value. A key seen for the first time starts at its target, so a
	 * freshly opened page does not animate every switch from off.
	 */
	private final Map<Object, Float> anims = new HashMap<>();
	private final long openedAt = System.currentTimeMillis();
	private long lastFrame;
	/** Seconds since the last frame, capped so a hitch cannot teleport things. */
	private float dt;
	/** Where the view is actually drawn; trails {@link #scroll}. */
	private float scrollPos;
	/** Where the rail highlight is actually drawn; trails the category index. */
	private float railPos;
	/** When the current view (category, detail, search) was entered. */
	private long contentSince = openedAt;
	private Object lastView;

	/** One drawn line: a card's on/off switch, or one of its settings. */
	private static final class Row {
		ModEntry entry;
		Option option;
		int height;
		int y;
	}

	private final List<Row> rows = new ArrayList<>();

	public MangoConfigScreen() {
		super(Text.literal(MangoConfig.NAME));
	}

	/** Opens straight onto one module's settings - the HUD editor's gear. */
	public MangoConfigScreen(HudModule target) {
		this();
		List<Category> all = MangoConfig.categories();
		for (int i = 0; i < all.size(); i++) {
			for (ModEntry entry : all.get(i).entries) {
				if (entry.module == target) {
					category = i;
					detail = entry;
					return;
				}
			}
		}
	}

	@Override
	protected void init() {
		panelW = Math.min(640, this.width - 16);
		panelH = Math.min(380, this.height - 16);
		left = (this.width - panelW) / 2;
		top = (this.height - panelH) / 2;
	}

	private List<Category> categories() {
		return MangoConfig.categories();
	}

	private Category current() {
		List<Category> all = categories();
		return all.get(Math.max(0, Math.min(all.size() - 1, category)));
	}

	private boolean searching() {
		return query.length() > 0;
	}

	/** What the grid shows: every category's matches while searching. */
	private List<ModEntry> gridEntries() {
		if (!searching()) return current().entries;
		List<ModEntry> out = new ArrayList<>();
		String q = query.toString();
		for (Category cat : categories()) {
			for (ModEntry entry : cat.entries) {
				if (entry.matches(q)) out.add(entry);
			}
		}
		return out;
	}

	private boolean gridShowing() {
		return detail == null && (searching() || current().grid());
	}

	private int contentX() {
		return left + RAIL_W;
	}

	private int contentW() {
		return panelW - RAIL_W;
	}

	private int viewTop() {
		return top + HEADER_H;
	}

	private int viewBottom() {
		return top + panelH - FOOTER_H;
	}

	private int cardW() {
		return (contentW() - PAD * 2 - GAP * (COLS - 1)) / COLS;
	}

	// --- rows (detail and plain-settings views) ------------------------------

	private void rebuildRows() {
		rows.clear();
		if (detail != null) {
			Row on = new Row();
			on.entry = detail;
			on.height = ROW_H;
			rows.add(on);
			for (Option option : detail.options) {
				Row row = new Row();
				row.option = option;
				row.height = option.hint.isEmpty() ? ROW_H : HINT_ROW_H;
				rows.add(row);
			}
		} else if (!gridShowing()) {
			for (Option option : current().options) {
				Row row = new Row();
				row.option = option;
				row.height = option.hint.isEmpty() ? ROW_H : HINT_ROW_H;
				rows.add(row);
			}
		}

		int y = viewTop() + 6 - Math.round(scrollPos);
		for (Row row : rows) {
			row.y = y;
			y += row.height;
		}
	}

	// --- animation helpers ---------------------------------------------------

	/** Eases `key` toward 1 while active, back toward 0 when not. */
	private float anim(Object key, boolean active, float speed) {
		float target = active ? 1f : 0f;
		Float held = anims.get(key);
		float v = held == null ? target : held;
		if (v != target) {
			v = Math.max(0f, Math.min(1f, v + (active ? 1f : -1f) * dt * speed));
		}
		anims.put(key, v);
		return v;
	}

	/** Ease-out cubic: fast off the mark, settling gently. */
	private static float ease(float t) {
		float u = 1f - Math.max(0f, Math.min(1f, t));
		return 1f - u * u * u;
	}

	private int contentHeight() {
		if (gridShowing()) {
			int count = gridEntries().size();
			int gridRows = (count + COLS - 1) / COLS;
			return gridRows == 0 ? 0 : PAD + gridRows * (CARD_H + GAP) - GAP + PAD;
		}
		int h = 12;
		for (Row row : rows) h += row.height;
		return h;
	}

	private int maxScroll() {
		return Math.max(0, contentHeight() - (viewBottom() - viewTop()));
	}

	// --- rendering -----------------------------------------------------------

	@Override
	public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
		long now = System.currentTimeMillis();
		dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1000f);
		lastFrame = now;

		// A change of view snaps the scroll and restarts the content slide.
		Object view = List.of(category, detail == null ? "" : detail, searching());
		if (!view.equals(lastView)) {
			lastView = view;
			contentSince = now;
			scrollPos = scroll;
		}

		rebuildRows();
		scroll = Math.max(0, Math.min(scroll, maxScroll()));
		scrollPos += (scroll - scrollPos) * Math.min(1f, dt * 14f);
		if (Math.abs(scroll - scrollPos) < 0.5f) scrollPos = scroll;
		rebuildRows();

		// The whole panel grows in from 92% as the scrim fades up.
		float open = ease(Math.min(1f, (now - openedAt) / 200f));
		ctx.fill(0, 0, this.width, this.height, Theme.withAlpha(Theme.SCRIM, Math.round(0xC0 * open)));

		Compat.push(ctx);
		float grow = 0.92f + 0.08f * open;
		Compat.translate(ctx, this.width / 2f, this.height / 2f);
		Compat.scale(ctx, grow, grow);
		Compat.translate(ctx, -this.width / 2f, -this.height / 2f);

		ctx.fill(left, top, left + panelW, top + panelH, Theme.SURFACE_1);
		ctx.fill(left, top, left + RAIL_W, top + panelH, Theme.SURFACE_2);

		drawHeader(ctx, mouseX, mouseY);
		drawRail(ctx, mouseX, mouseY);

		// A freshly entered view slides in from the right a touch.
		float in = ease(Math.min(1f, (now - contentSince) / 160f));
		String hover = null;
		ctx.enableScissor(contentX(), viewTop(), left + panelW, viewBottom());
		Compat.push(ctx);
		Compat.translate(ctx, Math.round((1f - in) * 14f), 0f);
		if (gridShowing()) {
			hover = drawGrid(ctx, mouseX, mouseY);
		} else {
			for (Row row : rows) {
				if (row.y + row.height < viewTop() || row.y > viewBottom()) continue;
				if (row.entry != null) drawEnabledRow(ctx, row);
				else drawOptionRow(ctx, row, mouseX, mouseY);
			}
		}
		Compat.pop(ctx);
		ctx.disableScissor();

		drawScrollbar(ctx);
		drawFooter(ctx, mouseX, mouseY, hover);

		Compat.pop(ctx);
		super.render(ctx, mouseX, mouseY, delta);
	}

	private String trim(String text, int width) {
		if (width <= 0) return "";
		return this.textRenderer.getWidth(text) <= width ? text : this.textRenderer.trimToWidth(text, width - 6) + "…";
	}

	// --- header --------------------------------------------------------------

	private int[] searchRect() {
		int w = Math.min(150, contentW() - PAD * 2);
		return new int[]{left + panelW - PAD - w, top + (HEADER_H - SEARCH_H) / 2, w, SEARCH_H};
	}

	/** Where the view's title sits; in the detail view it is the back button. */
	private int[] titleRect() {
		return new int[]{contentX() + PAD, top + 4, searchRect()[0] - contentX() - PAD * 2, HEADER_H - 8};
	}

	private void drawHeader(DrawContext ctx, int mouseX, int mouseY) {
		ctx.fill(left, top, left + panelW, top + HEADER_H, Theme.SURFACE_2);
		ctx.fill(left, top + HEADER_H - 1, left + panelW, top + HEADER_H, Theme.SURFACE_4);
		ctx.drawText(this.textRenderer, MangoConfig.NAME, left + PAD, top + 9, Theme.BRAND, false);
		ctx.drawText(this.textRenderer, "MangoClient", left + PAD, top + 21, Theme.TEXT_3, false);

		int[] t = titleRect();
		String name;
		String description;
		if (detail != null) {
			boolean hot = inside(mouseX, mouseY, t);
			name = "◂ " + detail.name;
			description = detail.description;
			ctx.drawText(this.textRenderer, trim(name, t[2]), t[0], top + 9,
				hot ? Theme.BRAND_HOVER : Theme.TEXT, false);
		} else if (searching()) {
			int count = gridEntries().size();
			name = "Search";
			description = count == 1 ? "1 mod matches." : count + " mods match.";
			ctx.drawText(this.textRenderer, name, t[0], top + 9, Theme.TEXT, false);
		} else {
			name = current().name;
			description = current().description;
			ctx.drawText(this.textRenderer, trim(name, t[2]), t[0], top + 9, Theme.TEXT, false);
		}
		ctx.drawText(this.textRenderer, trim(description, t[2]), t[0], top + 21, Theme.TEXT_3, false);

		drawSearchBox(ctx);
	}

	private void drawSearchBox(DrawContext ctx) {
		int[] r = searchRect();
		int border = searchFocused ? Theme.BRAND : Theme.SURFACE_4;
		ctx.fill(r[0] - 1, r[1] - 1, r[0] + r[2] + 1, r[1] + r[3] + 1, border);
		ctx.fill(r[0], r[1], r[0] + r[2], r[1] + r[3], Theme.SURFACE_1);

		int textX = r[0] + 5;
		int textY = r[1] + (r[3] - this.textRenderer.fontHeight) / 2 + 1;
		if (query.length() == 0 && !searchFocused) {
			ctx.drawText(this.textRenderer, trim("Search mods…", r[2] - 10), textX, textY, Theme.TEXT_3, false);
			return;
		}
		String text = query.toString();
		if (searchFocused && (System.currentTimeMillis() / 500) % 2 == 0) text += "_";
		ctx.drawText(this.textRenderer, trim(text, r[2] - 20), textX, textY, Theme.TEXT, false);
		if (query.length() > 0) {
			ctx.drawText(this.textRenderer, "×", r[0] + r[2] - 10, textY, Theme.TEXT_3, false);
		}
	}

	// --- rail ---------------------------------------------------------------

	/**
	 * Rail entries shrink rather than scroll or spill: there are few enough of
	 * them that they always fit if they are allowed to give up some padding,
	 * and a second scrolling list would be one too many.
	 */
	private int railItemH() {
		int available = viewBottom() - viewTop() - 6;
		return Math.max(12, Math.min(24, available / Math.max(1, categories().size())));
	}

	private void drawRail(DrawContext ctx, int mouseX, int mouseY) {
		List<Category> all = categories();
		int itemH = railItemH();
		int textY = (itemH - this.textRenderer.fontHeight) / 2 + 1;

		// One highlight that slides between entries rather than a fill that
		// jumps: the drawn position trails the chosen index.
		railPos += (category - railPos) * Math.min(1f, dt * 16f);
		if (Math.abs(category - railPos) < 0.01f) railPos = category;
		if (!searching()) {
			int hy = top + HEADER_H + 6 + Math.round(railPos * itemH);
			ctx.fill(left, hy, left + RAIL_W, hy + itemH, Theme.SURFACE_3);
			ctx.fill(left, hy, left + 2, hy + itemH, Theme.BRAND);
		}

		for (int i = 0; i < all.size(); i++) {
			int y = top + HEADER_H + 6 + i * itemH;
			boolean active = i == category && !searching();
			boolean hot = mouseX >= left && mouseX < left + RAIL_W && mouseY >= y && mouseY < y + itemH;
			float hovered = ease(anim(Map.entry("rail", i), hot && !active, 10f));
			if (hovered > 0f) {
				ctx.fill(left, y, left + RAIL_W, y + itemH,
					Theme.withAlpha(Theme.SURFACE_3, Math.round(hovered * 255)));
			}
			ctx.drawText(this.textRenderer, trim(all.get(i).name, RAIL_W - PAD * 2), left + PAD, y + textY,
				active ? Theme.TEXT : Theme.mix(Theme.TEXT_2, Theme.TEXT, hovered), false);
		}
	}

	// --- the grid ------------------------------------------------------------

	private int[] cardRect(int index) {
		int w = cardW();
		int col = index % COLS;
		int row = index / COLS;
		int x = contentX() + PAD + col * (w + GAP);
		int y = viewTop() + PAD + row * (CARD_H + GAP) - Math.round(scrollPos);
		return new int[]{x, y, w, CARD_H};
	}

	/** The switch at the bottom of a card, centred. */
	private int[] cardSwitchRect(int[] card) {
		return new int[]{card[0] + (card[2] - 18) / 2, card[1] + CARD_H - 14, 18, 10};
	}

	/** Draws every visible card; returns the hovered card's description. */
	private String drawGrid(DrawContext ctx, int mouseX, int mouseY) {
		List<ModEntry> entries = gridEntries();
		if (entries.isEmpty()) {
			String empty = searching() ? "No mod matches \"" + query + "\"." : "Nothing here yet.";
			ctx.drawText(this.textRenderer, trim(empty, contentW() - PAD * 2),
				contentX() + PAD, viewTop() + PAD + 4, Theme.TEXT_3, false);
			return null;
		}

		String hover = null;
		boolean inView = mouseY >= viewTop() && mouseY < viewBottom();
		for (int i = 0; i < entries.size(); i++) {
			int[] r = cardRect(i);
			if (r[1] + r[3] < viewTop() || r[1] > viewBottom()) continue;
			ModEntry entry = entries.get(i);
			boolean hot = inView && inside(mouseX, mouseY, r);
			if (hot) hover = entry.description;
			drawCard(ctx, entry, r, hot);
		}
		return hover;
	}

	private void drawCard(DrawContext ctx, ModEntry entry, int[] r, boolean hot) {
		boolean on = entry.enabled();
		// Hovering lifts the card a couple of pixels and fades in a brand-
		// coloured edge; both ease back out when the mouse leaves.
		float hovered = ease(anim(Map.entry(entry, "hover"), hot, 9f));
		int y = r[1] - Math.round(hovered * 2f);

		ctx.fill(r[0], y, r[0] + r[2], y + r[3], Theme.mix(Theme.SURFACE_2, Theme.SURFACE_3, hovered));
		if (hovered > 0f) {
			int glow = Theme.withAlpha(Theme.BRAND, Math.round(90 * hovered));
			ctx.fill(r[0], y, r[0] + r[2], y + 1, glow);
			ctx.fill(r[0], y + r[3] - 1, r[0] + r[2], y + r[3], glow);
			ctx.fill(r[0], y + 1, r[0] + 1, y + r[3] - 1, glow);
			ctx.fill(r[0] + r[2] - 1, y + 1, r[0] + r[2], y + r[3] - 1, glow);
		}
		if (on) ctx.fill(r[0], y, r[0] + r[2], y + 1, Theme.BRAND);

		// The mod's icon on a plate: a Minecraft item or GUI sprite where one
		// fits, or the mod's initial where none does.
		int plate = 20;
		int px = r[0] + (r[2] - plate) / 2;
		int py = y + 6;
		ctx.fill(px, py, px + plate, py + plate, on ? Theme.BRAND_QUIET : Theme.SURFACE_4);
		Icons.Icon icon = entry.icon;
		if (icon != null && icon.sprite() != null) {
			Compat.drawGuiTexture(ctx, icon.sprite(),
				px + (plate - icon.width()) / 2, py + (plate - icon.height()) / 2, icon.width(), icon.height());
		} else if (icon != null) {
			ctx.drawItem(icon.stack(), px + 2, py + 2);
		} else {
			String initial = entry.name.substring(0, 1);
			ctx.drawText(this.textRenderer, initial,
				px + (plate - this.textRenderer.getWidth(initial)) / 2,
				py + (plate - this.textRenderer.fontHeight) / 2 + 1,
				on ? Theme.BRAND : Theme.TEXT_3, false);
		}

		String name = trim(entry.name, r[2] - 8);
		ctx.drawText(this.textRenderer, name,
			r[0] + (r[2] - this.textRenderer.getWidth(name)) / 2, y + 30,
			on ? Theme.TEXT : Theme.TEXT_2, false);

		int[] s = cardSwitchRect(new int[]{r[0], y, r[2], r[3]});
		drawSwitch(ctx, Map.entry(entry, "switch"), s[0], s[1], s[2], on);
	}

	// --- rows ---------------------------------------------------------------

	/** The first row of a detail view: the mod's own on/off switch. */
	private void drawEnabledRow(DrawContext ctx, Row row) {
		int x = contentX() + PAD;
		int w = contentW() - PAD * 2;
		ctx.drawText(this.textRenderer, "Enabled", x, row.y + 7, Theme.TEXT, false);
		drawSwitch(ctx, Map.entry(row.entry, "switch"), x + w - SWITCH_W, row.y + 6, SWITCH_W, row.entry.enabled());
	}

	private void drawOptionRow(DrawContext ctx, Row row, int mouseX, int mouseY) {
		Option option = row.option;
		int x = contentX() + PAD;
		int w = contentW() - PAD * 2;
		int ty = row.y + 7;

		// The row under the mouse breathes in a faint highlight.
		boolean hot = mouseX >= x - 4 && mouseX < x + w + 4
			&& mouseY >= row.y && mouseY < row.y + row.height
			&& mouseY >= viewTop() && mouseY < viewBottom();
		float hovered = ease(anim(Map.entry(option, "row"), hot, 10f));
		if (hovered > 0f) {
			ctx.fill(x - 4, row.y, x + w + 4, row.y + row.height - 2,
				Theme.withAlpha(Theme.SURFACE_3, Math.round(150 * hovered)));
		}

		int controlW = controlWidth(option);
		int labelW = w - controlW - 10;
		ctx.drawText(this.textRenderer, trim(option.label, labelW), x, ty, Theme.TEXT_2, false);
		if (!option.hint.isEmpty()) {
			ctx.drawText(this.textRenderer, trim(option.hint, w), x, ty + 10, Theme.TEXT_3, false);
		}

		int right = x + w;
		if (option instanceof Option.Bool b) {
			drawSwitch(ctx, Map.entry(b, "switch"), right - SWITCH_W, row.y + 6, SWITCH_W, b.get());
		} else if (option instanceof Option.Range r) {
			String value = r.display();
			ctx.drawText(this.textRenderer, value, right - this.textRenderer.getWidth(value), ty, Theme.TEXT_3, false);
			int trackX = right - VALUE_W - TRACK_W;
			int trackY = row.y + 10;
			ctx.fill(trackX, trackY, trackX + TRACK_W, trackY + 3, Theme.SURFACE_4);
			int fill = Math.round(TRACK_W * r.fraction());
			ctx.fill(trackX, trackY, trackX + fill, trackY + 3, Theme.BRAND);
			int knob = trackX + Math.max(0, Math.min(TRACK_W - 4, fill - 2));
			ctx.fill(knob, trackY - 3, knob + 4, trackY + 6, Theme.BRAND_HOVER);
		} else if (option instanceof Option.Colour c) {
			ctx.fill(right - 15, row.y + 5, right + 1, row.y + 17, Theme.SURFACE_4);
			ctx.fill(right - 14, row.y + 6, right, row.y + 16, 0xFF000000 | (c.value & 0x00FFFFFF));
		} else if (option instanceof Option.Choice ch) {
			String value = trim(ch.value(), controlW);
			ctx.drawText(this.textRenderer, value, right - this.textRenderer.getWidth(value), ty, Theme.BRAND, false);
		} else if (option instanceof Option.Key k) {
			String value = trim(k.display(), controlW);
			ctx.drawText(this.textRenderer, value, right - this.textRenderer.getWidth(value), ty,
				k.listening ? Theme.BRAND_HOVER : Theme.BRAND, false);
		} else if (option instanceof Option.Readout ro) {
			String value = trim(ro.value.get(), controlW);
			ctx.drawText(this.textRenderer, value, right - this.textRenderer.getWidth(value), ty, Theme.TEXT_3, false);
		}
	}

	private int controlWidth(Option option) {
		if (option instanceof Option.Bool) return SWITCH_W;
		if (option instanceof Option.Range) return TRACK_W + VALUE_W;
		if (option instanceof Option.Colour) return 16;
		return 110;
	}

	/** The knob glides between its ends; the track cross-fades behind it. */
	private void drawSwitch(DrawContext ctx, Object key, int x, int y, int w, boolean on) {
		float p = ease(anim(key, on, 12f));
		int h = w == SWITCH_W ? 11 : 10;
		int knobW = w == SWITCH_W ? 7 : 6;
		ctx.fill(x, y, x + w, y + h, Theme.mix(Theme.SURFACE_4, Theme.BRAND, p));
		int off = x + 2;
		int full = x + w - knobW - 2;
		int knob = Math.round(off + (full - off) * p);
		ctx.fill(knob, y + 2, knob + knobW, y + h - 2, Theme.mix(Theme.TEXT_3, Theme.BRAND_FG, p));
	}

	private void drawScrollbar(DrawContext ctx) {
		int max = maxScroll();
		if (max <= 0) return;
		int viewH = viewBottom() - viewTop();
		int barH = Math.max(20, Math.round(viewH * (viewH / (float) contentHeight())));
		int barY = viewTop() + Math.round((viewH - barH) * Math.max(0f, Math.min(1f, scrollPos / max)));
		ctx.fill(left + panelW - 4, viewTop(), left + panelW - 1, viewBottom(), Theme.SURFACE_2);
		ctx.fill(left + panelW - 4, barY, left + panelW - 1, barY + barH, Theme.SURFACE_4);
	}

	// --- footer -------------------------------------------------------------

	private int[] editorRect() {
		return new int[]{left + panelW - 128, top + panelH - 24, 116, 18};
	}

	private void drawFooter(DrawContext ctx, int mouseX, int mouseY, String hover) {
		ctx.fill(left, top + panelH - FOOTER_H, left + panelW, top + panelH, Theme.SURFACE_2);
		ctx.fill(left, top + panelH - FOOTER_H, left + panelW, top + panelH - FOOTER_H + 1, Theme.SURFACE_4);

		int[] r = editorRect();
		String note = hover != null ? hover : "MangoClient";
		ctx.drawText(this.textRenderer, trim(note, r[0] - left - PAD * 2), left + PAD, top + panelH - 20,
			Theme.TEXT_3, false);

		// A pill rather than a slab: nicked corners, a move glyph, and a lift
		// with a soft glow while hovered.
		boolean hot = inside(mouseX, mouseY, r);
		float hovered = ease(anim("editorButton", hot, 10f));
		int by = r[1] - Math.round(hovered);
		int fill = Theme.mix(Theme.BRAND, Theme.BRAND_HOVER, hovered);
		ctx.fill(r[0], by, r[0] + r[2], by + r[3], fill);
		int corner = Theme.SURFACE_2;
		ctx.fill(r[0], by, r[0] + 1, by + 1, corner);
		ctx.fill(r[0] + r[2] - 1, by, r[0] + r[2], by + 1, corner);
		ctx.fill(r[0], by + r[3] - 1, r[0] + 1, by + r[3], corner);
		ctx.fill(r[0] + r[2] - 1, by + r[3] - 1, r[0] + r[2], by + r[3], corner);
		if (hovered > 0f) {
			int glow = Theme.withAlpha(Theme.BRAND_HOVER, Math.round(110 * hovered));
			ctx.fill(r[0] - 1, by + 1, r[0], by + r[3] - 1, glow);
			ctx.fill(r[0] + r[2], by + 1, r[0] + r[2] + 1, by + r[3] - 1, glow);
			ctx.fill(r[0] + 1, by - 1, r[0] + r[2] - 1, by, glow);
			ctx.fill(r[0] + 1, by + r[3], r[0] + r[2] - 1, by + r[3] + 1, glow);
		}

		String label = "Arrange HUD";
		int iconW = 9;
		int textW = this.textRenderer.getWidth(label);
		int cx = r[0] + (r[2] - textW - iconW - 5) / 2;
		int cy = by + r[3] / 2;
		// A little move cross drawn from fills, so no font has to carry it.
		ctx.fill(cx, cy - 1, cx + iconW, cy + 1, Theme.BRAND_FG);
		ctx.fill(cx + iconW / 2 - 1, cy - iconW / 2, cx + iconW / 2 + 1, cy + iconW / 2 + 1, Theme.BRAND_FG);
		ctx.drawText(this.textRenderer, label, cx + iconW + 5, by + 5, Theme.BRAND_FG, false);
	}

	// --- input --------------------------------------------------------------

	private static boolean inside(double mx, double my, int[] r) {
		return mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
	}

	@Override
	protected boolean onMouseDown(double mx, double my) {

		// A click anywhere gives up on a key that was being listened for; the
		// row itself starts listening again below if that is what was clicked.
		stopListening();

		int[] search = searchRect();
		if (inside(mx, my, search)) {
			if (query.length() > 0 && mx >= search[0] + search[2] - 14) {
				query.setLength(0);
				scroll = 0;
			}
			searchFocused = true;
			return true;
		}
		searchFocused = false;

		if (inside(mx, my, editorRect())) {
			MinecraftClient.getInstance().setScreen(new HudEditorScreen(this));
			return true;
		}

		if (detail != null && inside(mx, my, titleRect())) {
			closeDetail();
			return true;
		}

		List<Category> all = categories();
		if (mx >= left && mx < left + RAIL_W && my >= top + HEADER_H) {
			int itemH = railItemH();
			for (int i = 0; i < all.size(); i++) {
				int y = top + HEADER_H + 6 + i * itemH;
				if (my >= y && my < y + itemH) {
					category = i;
					detail = null;
					query.setLength(0);
					scroll = 0;
					return true;
				}
			}
		}

		if (my < viewTop() || my > viewBottom() || mx < contentX()) {
			return false;
		}

		if (gridShowing()) {
			return clickGrid(mx, my);
		}

		rebuildRows();
		for (Row row : rows) {
			if (my < row.y || my >= row.y + row.height) continue;
			if (row.entry != null) {
				int right = contentX() + PAD + contentW() - PAD * 2;
				if (mx >= right - SWITCH_W - 6) row.entry.toggle();
				return true;
			}
			return clickOption(row, mx);
		}
		return false;
	}

	private boolean clickGrid(double mx, double my) {
		List<ModEntry> entries = gridEntries();
		for (int i = 0; i < entries.size(); i++) {
			int[] r = cardRect(i);
			if (!inside(mx, my, r)) continue;
			ModEntry entry = entries.get(i);
			if (inside(mx, my, grow(cardSwitchRect(r), 3))) {
				entry.toggle();
			} else {
				detail = entry;
				scroll = 0;
			}
			return true;
		}
		return true;
	}

	/** A slightly larger target than the switch is drawn: it is small. */
	private static int[] grow(int[] r, int by) {
		return new int[]{r[0] - by, r[1] - by, r[2] + by * 2, r[3] + by * 2};
	}

	private void closeDetail() {
		detail = null;
		scroll = 0;
	}

	private boolean clickOption(Row row, double mx) {
		Option option = row.option;
		int x = contentX() + PAD;
		int right = x + contentW() - PAD * 2;

		if (option instanceof Option.Bool b) {
			if (mx >= right - SWITCH_W - 6) {
				b.toggle();
				MangoConfig.save();
			}
			return true;
		}
		if (option instanceof Option.Range r) {
			int trackX = right - VALUE_W - TRACK_W;
			if (mx >= trackX - 4 && mx <= trackX + TRACK_W + 4) {
				draggingRange = r;
				r.setFraction((mx - trackX) / (double) TRACK_W);
			}
			return true;
		}
		if (option instanceof Option.Colour c) {
			if (mx >= right - 20) {
				c.value = nextSwatch(c.value);
				MangoConfig.save();
			}
			return true;
		}
		if (option instanceof Option.Choice ch) {
			if (mx >= right - 110) {
				ch.next();
				MangoConfig.save();
			}
			return true;
		}
		if (option instanceof Option.Key k) {
			if (mx >= right - 110) startListening(k);
			return true;
		}
		return true;
	}

	private void startListening(Option.Key key) {
		stopListening();
		listeningKey = key;
		key.listening = true;
	}

	private void stopListening() {
		if (listeningKey != null) listeningKey.listening = false;
		listeningKey = null;
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
	protected boolean onMouseDrag(double mx, double my, double dx, double dy) {
		if (draggingRange != null) {
			int right = contentX() + contentW() - PAD;
			int trackX = right - VALUE_W - TRACK_W;
			draggingRange.setFraction((mx - trackX) / (double) TRACK_W);
			return true;
		}
		return false;
	}

	@Override
	protected boolean onMouseUp(double mx, double my) {
		if (draggingRange != null) {
			draggingRange = null;
			MangoConfig.save();
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseScrolled(double mouseX, double mouseY, double horizontal, double vertical) {
		if (mouseX >= contentX() && mouseY >= viewTop() && mouseY <= viewBottom()) {
			scroll = Math.max(0, Math.min(maxScroll(), scroll - (int) Math.round(vertical * 18)));
			return true;
		}
		return super.mouseScrolled(mouseX, mouseY, horizontal, vertical);
	}

	@Override
	protected boolean onCharTyped(char chr) {
		if (searchFocused && query.length() < QUERY_MAX) {
			query.append(chr);
			detail = null;
			scroll = 0;
			return true;
		}
		return false;
	}

	@Override
	protected boolean onKeyDown(int key) {
		if (searchFocused) {
			switch (key) {
				case GLFW.GLFW_KEY_BACKSPACE -> {
					if (query.length() > 0) query.setLength(query.length() - 1);
					scroll = 0;
				}
				case GLFW.GLFW_KEY_ESCAPE -> {
					query.setLength(0);
					searchFocused = false;
					scroll = 0;
				}
				case GLFW.GLFW_KEY_ENTER, GLFW.GLFW_KEY_KP_ENTER -> searchFocused = false;
				default -> {
				}
			}
			// Swallow everything while typing, or "e" would close the screen.
			return true;
		}
		if (listeningKey != null) {
			// Escape backs out of rebinding rather than closing the screen,
			// which is the one press nobody means as a binding.
			if (key != GLFW.GLFW_KEY_ESCAPE) {
				listeningKey.value = key;
				MangoConfig.save();
			}
			stopListening();
			return true;
		}
		if (key == GLFW.GLFW_KEY_ESCAPE) {
			// Escape peels back one layer at a time: settings, then the screen.
			if (detail != null) {
				closeDetail();
				return true;
			}
			this.close();
			return true;
		}
		return false;
	}

	@Override
	public void close() {
		stopListening();
		MangoConfig.save();
		// Half these rows write straight into Minecraft's options; make them stick.
		GameSettings.persist();
		super.close();
	}
}
