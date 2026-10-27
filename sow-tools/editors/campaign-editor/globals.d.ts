/* eslint-disable no-var -- these must be `var` so they appear on typeof globalThis, where the page scripts assign them */
// Globals that the editor pages read from scripts the page loads before its own script.

interface AtlasEntity {
  id: string;
  name: string;
  role: string;
  region: string;
  avatar?: string;
  territory_color?: string;
}

interface AtlasEntityPickerApi {
  attach(options: {
    input: HTMLInputElement;
    results: HTMLElement;
    status: HTMLElement;
    getEntities: () => AtlasEntity[] | Promise<AtlasEntity[]>;
    onChange?: (entity: AtlasEntity | null) => void;
  }): {
    getSelected(): AtlasEntity | null;
    select(id: string): Promise<boolean>;
    clear(): void;
  };
  resolve(entities: AtlasEntity[], value: string): AtlasEntity | null;
  search(entities: AtlasEntity[], value: string): AtlasEntity[];
}

declare var SOWCampaign: typeof import("../../../sow-web/shell/sow-campaign.ts").default;
declare var SOWCampaignMapPreview: typeof import("./map-preview.ts").default;
declare var SOWAtlasEntityPicker: AtlasEntityPickerApi;
