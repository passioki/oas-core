import time, numpy as np, cv2, sys
rng = np.random.default_rng(7)
H, W = 720, 1280
frame = rng.integers(0,256,size=(H,W),dtype=np.uint8)
tpls = [rng.integers(0,256,size=(46,56),dtype=np.uint8) for _ in range(50)]
rois = [(40 + (i*23)%1000, 30 + (i*17)%500, 160, 140) for i in range(50)]
# warmup
for i in range(5):
    x,y,_,_ = rois[i]; cv2.matchTemplate(frame[y:y+140, x:x+160], tpls[i], cv2.TM_CCOEFF_NORMED)
t0=time.perf_counter()
for i in range(50):
    x,y,_,_ = rois[i]
    cv2.matchTemplate(frame[y:y+140, x:x+160], tpls[i], cv2.TM_CCOEFF_NORMED)
dt=time.perf_counter()-t0
print(f"cv2 50 templates x 160x140 ROI: {dt*1000:.1f} ms total, {dt*1000/50:.2f} ms/template")
