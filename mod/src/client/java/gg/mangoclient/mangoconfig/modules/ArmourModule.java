package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.item.ItemStack;

import java.util.ArrayList;
import java.util.List;

/**
 * The armour a player is wearing, with what is left of each piece.
 *
 * Durability is the whole point of the module, so it is shown as a number the
 * player can read at a glance rather than as vanilla's small damage bar.
 */
public class ArmourModule extends HudModule {
	private static final EquipmentSlot[] ARMOUR = {
		EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET,
	};

	private final Option.Choice direction =
		new Option.Choice("direction", "Ausrichtung", 0, "Senkrecht", "Waagerecht");
	private final Option.Bool showHeld = new Option.Bool("held", "Hand mit anzeigen", true);
	private final Option.Bool showDurability = new Option.Bool("durability", "Haltbarkeit als Zahl", true);
	private final Option.Bool hideEmpty = new Option.Bool("hideEmpty", "Leere Plätze ausblenden", true);
	private final Option.Bool warnLow = new Option.Bool("warnLow", "Niedrige Haltbarkeit färben", true);

	public ArmourModule() {
		super("armour", "Rüstung", "Getragene Rüstung samt Haltbarkeit.", true, 0.01f, 0.35f);
		options.add(direction);
		options.add(showHeld);
		options.add(showDurability);
		options.add(hideEmpty);
		options.add(warnLow);
	}

	private List<ItemStack> stacks(MinecraftClient mc, boolean preview) {
		List<ItemStack> out = new ArrayList<>();
		if (mc.player == null) return out;
		for (EquipmentSlot slot : ARMOUR) {
			ItemStack stack = mc.player.getEquippedStack(slot);
			if (stack.isEmpty() && hideEmpty.value && !preview) continue;
			out.add(stack);
		}
		if (showHeld.value) {
			ItemStack held = mc.player.getEquippedStack(EquipmentSlot.MAINHAND);
			if (!held.isEmpty() || !hideEmpty.value || preview) out.add(held);
		}
		return out;
	}

	/** Widest durability label, so the column does not jitter as items wear. */
	private int labelWidth(MinecraftClient mc, TextRenderer font, boolean preview) {
		if (!showDurability.value) return 0;
		int max = 0;
		for (ItemStack stack : stacks(mc, preview)) {
			max = Math.max(max, font.getWidth(durability(stack)));
		}
		return max == 0 ? 0 : max + 4;
	}

	private String durability(ItemStack stack) {
		if (stack.isEmpty()) return "-";
		if (!stack.isDamageable()) return "";
		return String.valueOf(stack.getMaxDamage() - stack.getDamage());
	}

	private int durabilityColour(ItemStack stack) {
		if (!warnLow.value || stack.isEmpty() || !stack.isDamageable()) return colour.value;
		float left = 1f - (stack.getDamage() / (float) stack.getMaxDamage());
		if (left <= 0.1f) return 0xFFFF4B4B;
		if (left <= 0.25f) return 0xFFF2A53C;
		return colour.value;
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		int count = Math.max(1, stacks(mc, false).size());
		if (direction.is("Waagerecht")) return count * 18;
		return 18 + labelWidth(mc, font, false);
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		int count = Math.max(1, stacks(mc, false).size());
		return direction.is("Waagerecht") ? 18 : count * 18;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		List<ItemStack> items = stacks(mc, preview);
		int textOffset = (16 - font.fontHeight) / 2 + 1;

		for (int i = 0; i < items.size(); i++) {
			ItemStack stack = items.get(i);
			int px = direction.is("Waagerecht") ? i * 18 : 0;
			int py = direction.is("Waagerecht") ? 0 : i * 18;

			if (!stack.isEmpty()) {
				ctx.drawItem(stack, px, py);
				ctx.drawStackOverlay(font, stack, px, py);
			}
			if (showDurability.value && !direction.is("Waagerecht")) {
				String label = durability(stack);
				if (!label.isEmpty()) {
					ctx.drawText(font, label, px + 18, py + textOffset, durabilityColour(stack), shadow.value);
				}
			}
		}
	}
}
