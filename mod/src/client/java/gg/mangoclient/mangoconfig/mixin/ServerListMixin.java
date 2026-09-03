package gg.mangoclient.mangoconfig.mixin;

import java.util.List;

import net.minecraft.client.network.ServerInfo;
import net.minecraft.client.option.ServerList;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Keeps MangoClient partner servers in the vanilla multiplayer list while
 * leaving every player-created entry and all normal list behaviour untouched. */
@Mixin(ServerList.class)
public abstract class ServerListMixin {
	@Unique private static final String MANGO_PARTNER_NAME = "§6★ §rVincentVanilla";
	@Unique private static final String MANGO_PARTNER_ADDRESS = "vincentvanilla.net";

	@Shadow @Final private List<ServerInfo> servers;
	@Shadow public abstract ServerInfo get(int index);
	@Shadow public abstract void saveFile();

	private static String mangoconfig$normalAddress(String address) {
		if (address == null) return "";
		String value = address.trim().toLowerCase(java.util.Locale.ROOT);
		if (value.endsWith(".")) value = value.substring(0, value.length() - 1);
		if (value.endsWith(":25565")) value = value.substring(0, value.length() - 6);
		return value;
	}

	private static boolean mangoconfig$isPartner(ServerInfo server) {
		return server != null && MANGO_PARTNER_ADDRESS.equals(mangoconfig$normalAddress(server.address));
	}

	/** Return whether the persisted list needs another save. */
	private boolean mangoconfig$pinPartner() {
		boolean changed = servers.isEmpty() || !mangoconfig$isPartner(servers.get(0));
		ServerInfo partner = null;
		int matches = 0;
		for (ServerInfo candidate : servers) {
			if (!mangoconfig$isPartner(candidate)) continue;
			matches++;
			if (partner == null) partner = candidate;
		}
		servers.removeIf(ServerListMixin::mangoconfig$isPartner);
		if (partner == null) {
			partner = new ServerInfo(MANGO_PARTNER_NAME, MANGO_PARTNER_ADDRESS, ServerInfo.ServerType.OTHER);
		}
		changed |= matches != 1
			|| !MANGO_PARTNER_NAME.equals(partner.name)
			|| !MANGO_PARTNER_ADDRESS.equals(partner.address);
		partner.name = MANGO_PARTNER_NAME;
		partner.address = MANGO_PARTNER_ADDRESS;
		servers.add(0, partner);
		return changed;
	}

	@Inject(method = "loadFile", at = @At("RETURN"))
	private void mangoconfig$pinAfterLoad(CallbackInfo ci) {
		if (mangoconfig$pinPartner()) saveFile();
	}

	@Inject(method = "saveFile", at = @At("HEAD"))
	private void mangoconfig$pinBeforeSave(CallbackInfo ci) {
		mangoconfig$pinPartner();
	}

	@Inject(method = "swapEntries", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$keepPartnerFirst(int first, int second, CallbackInfo ci) {
		if (mangoconfig$isPartner(get(first)) || mangoconfig$isPartner(get(second))) ci.cancel();
	}

	@Inject(method = "remove", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$keepPartnerPresent(ServerInfo server, CallbackInfo ci) {
		if (mangoconfig$isPartner(server)) ci.cancel();
	}

	@Inject(method = "set", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$keepPartnerFixed(int index, ServerInfo replacement, CallbackInfo ci) {
		if (mangoconfig$isPartner(get(index))) ci.cancel();
	}
}
