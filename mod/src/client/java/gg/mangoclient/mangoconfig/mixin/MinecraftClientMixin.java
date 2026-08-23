package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.RPCManager;
import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The one hook the Discord presence needs.
 *
 * The HUD hook cannot carry it: that only fires with a world on screen, and
 * sitting in the main menu is a state Discord should show as much as any other.
 * The client tick runs either way, and all this does with it is hand over the
 * client - the work of deciding and sending is not done here.
 */
@Mixin(MinecraftClient.class)
public class MinecraftClientMixin {
	@Inject(method = "tick", at = @At("HEAD"))
	private void mangoconfig$rpcTick(CallbackInfo ci) {
		RPCManager.onClientTick((MinecraftClient) (Object) this);
	}
}
