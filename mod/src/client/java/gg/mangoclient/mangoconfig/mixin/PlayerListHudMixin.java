package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Presence;
import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.gui.hud.PlayerListHud;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The grey mango in the tab list: every name vanilla asks for goes through
 * here, and a MangoClient player's comes back with the badge in front.
 */
@Mixin(PlayerListHud.class)
public abstract class PlayerListHudMixin {

	@Inject(method = "getPlayerName", at = @At("RETURN"), cancellable = true)
	private void mangoconfig$badge(PlayerListEntry entry, CallbackInfoReturnable<Text> cir) {
		if (Presence.badgeEnabled() && Presence.isMango(Compat.profileId(entry.getProfile()))) {
			cir.setReturnValue(Presence.withBadge(cir.getReturnValue()));
		}
	}
}
