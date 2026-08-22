package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.entity.effect.StatusEffectInstance;

import java.util.ArrayList;
import java.util.List;

public class PotionsModule extends HudModule {
	private static final String[] ROMAN = {"", " II", " III", " IV", " V", " VI", " VII", " VIII"};

	private final Option.Bool showDuration = new Option.Bool("duration", "Show remaining time", true);
	private final Option.Range warnAt = new Option.Range("warn", "Warn below", 10, 0, 60, " s");

	public PotionsModule() {
		super("potions", "Effects", "Active status effects.", false, 0.86f, 0.18f);
		options.add(showDuration);
		options.add(warnAt);
	}

	private List<StatusEffectInstance> effects(MinecraftClient mc, boolean preview) {
		if (mc.player == null) return new ArrayList<>();
		return new ArrayList<>(mc.player.getStatusEffects());
	}

	private String label(StatusEffectInstance effect) {
		String name = effect.getEffectType().value().getName().getString();
		int amp = Math.max(0, Math.min(ROMAN.length - 1, effect.getAmplifier()));
		String text = name + ROMAN[amp];
		if (!showDuration.value) return text;
		return text + "  " + time(effect.getDuration());
	}

	private static String time(int ticks) {
		int seconds = ticks / 20;
		return String.format("%d:%02d", seconds / 60, seconds % 60);
	}

	private int colourFor(StatusEffectInstance effect) {
		if (effect.getDuration() / 20 <= warnAt.value) return 0xFFF2A53C;
		return colour.value;
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		int w = 0;
		for (StatusEffectInstance effect : effects(mc, false)) {
			w = Math.max(w, font.getWidth(label(effect)));
		}
		return Math.max(w, 1);
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return Math.max(1, effects(mc, false).size()) * (font.fontHeight + 1);
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		List<StatusEffectInstance> list = effects(mc, preview);
		if (list.isEmpty()) {
			if (preview) text(ctx, font, "No effects", 0, 0);
			return;
		}
		for (int i = 0; i < list.size(); i++) {
			StatusEffectInstance effect = list.get(i);
			ctx.drawText(font, label(effect), 0, i * (font.fontHeight + 1), colourFor(effect), shadow.value);
		}
	}
}
