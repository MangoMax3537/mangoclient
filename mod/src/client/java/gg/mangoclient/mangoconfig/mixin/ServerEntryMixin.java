package gg.mangoclient.mangoconfig.mixin;

import net.minecraft.client.gui.screen.multiplayer.MultiplayerServerListWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Vanilla moves a multiplayer entry twice: once in ServerList and once in the
 * visible widget. Cancelling only the former leaves a misleading row that still
 * appears movable until the screen is reopened. Slot zero belongs to the fixed
 * partner, so neither the partner nor another server may swap through it.
 */
@Mixin(MultiplayerServerListWidget.ServerEntry.class)
public class ServerEntryMixin {
	@Inject(method = "swapEntries", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$keepPartnerSlotFixed(int from, int to, CallbackInfo ci) {
		if (from == 0 || to == 0) ci.cancel();
	}
}
