const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const MAX_IMAGE_SIDE = 256;
const MAX_SAVED_CHARACTERS = 512 * 1024;
const SUPPORTED_TYPES = new Set(["image/png", "image/jpeg", "image/webp"]);

export function isProjectImage(value: string | null | undefined): boolean {
  return Boolean(value?.startsWith("data:image/png;base64,"));
}

export async function readProjectImage(file: File): Promise<string> {
  if (!SUPPORTED_TYPES.has(file.type)) {
    throw new Error("Elige una imagen PNG, JPG o WebP.");
  }
  if (file.size > MAX_SOURCE_BYTES) {
    throw new Error("La imagen debe pesar como máximo 5 MB.");
  }
  const source = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => typeof reader.result === "string"
      ? resolve(reader.result)
      : reject(new Error("No se pudo leer la imagen."));
    reader.onerror = () => reject(new Error("No se pudo leer la imagen."));
    reader.onabort = () => reject(new Error("La lectura de la imagen se canceló."));
    reader.readAsDataURL(file);
  });
  const image = await new Promise<HTMLImageElement>((resolve, reject) => {
    const candidate = new Image();
    candidate.onload = () => resolve(candidate);
    candidate.onerror = () => reject(new Error("El archivo no contiene una imagen válida."));
    candidate.src = source;
  });
  if (!image.naturalWidth || !image.naturalHeight || image.naturalWidth * image.naturalHeight > 40_000_000) {
    throw new Error("La imagen es demasiado grande. Elige una de hasta 40 megapíxeles.");
  }
  const scale = Math.min(1, MAX_IMAGE_SIDE / Math.max(image.naturalWidth, image.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("No se pudo preparar la imagen.");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const result = canvas.toDataURL("image/png");
  if (!isProjectImage(result) || result.length > MAX_SAVED_CHARACTERS) {
    throw new Error("No se pudo reducir la imagen. Prueba con un archivo más pequeño.");
  }
  return result;
}
