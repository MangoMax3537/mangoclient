package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.item.ArrowItem;
import net.minecraft.item.ItemStack;

import java.util.ArrayList;
import java.util.List;

/**
 * How much of what the player is carrying.
 *
 * Counting arrows across the whole inventory is the point: vanilla only shows
 * the stack in a slot, and running dry mid-fight is how bow fights are lost.
 */
public class ItemsModule extends TextModule {
	private final Option.Bool arrows = new Option.Bool("arrows", "Arrows", true);
	private final Option.Bool held = new Option.Bool("held", "Held stack", false);
	private final Option.Bool offhand = new Option.Bool("offhand", "Off-hand", false);

	public ItemsModule() {
		super("items", "Items", "What you are carrying, counted.", false, 0.86f, 0.14f);
		options.add(arrows);
		options.add(held);
		options.add(offhand);
	}

	private int arrowCount(ClientPlayerEntity player) {
		int total = 0;
		for (ItemStack stack : player.getInventory().getMainStacks()) {
			if (stack.getItem() instanceof ArrowItem) total += stack.getCount();
		}
		ItemStack off = player.getOffHandStack();
		if (off.getItem() instanceof ArrowItem) total += off.getCount();
		return total;
	}

	private static String describe(ItemStack stack) {
		if (stack.isEmpty()) return "empty";
		String name = stack.getName().getString();
		return stack.getCount() > 1 ? stack.getCount() + "x " + name : name;
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		ClientPlayerEntity player = mc.player;
		if (player == null) {
			if (preview) out.add("64 arrows");
			return out;
		}

		if (arrows.value) {
			int count = arrowCount(player);
			out.add(count + (count == 1 ? " arrow" : " arrows"));
		}
		if (held.value) out.add(describe(player.getMainHandStack()));
		if (offhand.value) out.add(describe(player.getOffHandStack()));
		if (out.isEmpty() && preview) out.add("64 arrows");
		return out;
	}
}
