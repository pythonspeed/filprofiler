import time
import sys

import numpy as np

from skimage import data
from skimage.registration import phase_cross_correlation
from scipy.ndimage import fourier_shift
from skimage.transform import rescale

start = time.time()
image = data.camera()

if len(sys.argv) > 1:
    scale = int(sys.argv[1])
else:
    scale = 4
image = rescale(image, scale, anti_aliasing=True)

shift = (-22.4, 13.32)
# The shift corresponds to the pixel offset relative to the reference image
offset_image = fourier_shift(np.fft.fftn(image), shift)
offset_image = np.fft.ifftn(offset_image)
print(f"Known offset (y, x): {shift}")

# pixel precision first
shift, error, diffphase = phase_cross_correlation(image, offset_image)

# Show the output of a cross-correlation to show what the algorithm is
# doing behind the scenes
image_product = np.fft.fft2(image) * np.fft.fft2(offset_image).conj()
cc_image = np.fft.fftshift(np.fft.ifft2(image_product))

print(f"Detected pixel offset (y, x): {shift}")
print(f"Elapsed {time.time() - start}")
